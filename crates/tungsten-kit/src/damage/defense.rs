//! Defences between a damage request and health: resistances by damage
//! tag, and a shield that takes damage first and recharges after a delay.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tungsten_core::{Inspectable, Time, World};

use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, SHMUP, TOP_DOWN, round_trip,
};
use crate::foundation::tags::{Tags, valid_tag};

/// [`HealthPlugin`](crate::HealthPlugin)'s system that recharges shields, in
/// the tick phase.
pub const SHIELD_TICK: &str = "shield_tick";

/// The scene components the health plugin registers for the defences.
pub(crate) const RESIST: &str = "resist";
pub(crate) const SHIELD: &str = "shield";

/// The largest resistance factor.
const MAX_FACTOR: f32 = 1000.0;

/// An entity's resistances: a factor per damage tag, which multiplies the
/// amount of a [`DamageRequest`](crate::DamageRequest) whose tags carry
/// that tag, exactly or as a dotted ancestor: a factor for `damage` applies
/// to a `damage.fire` request too. A factor of 0 is immunity: the request
/// is ignored before any product. The matching factors multiply in `f64`
/// and the amount they make is clamped to `f32::MAX`, so every amount stays
/// finite.
///
/// Scene name `resist`: `{ "damage.fire": 0.5, "damage.poison": 0 }`, each
/// key a declared tag, each factor finite in `[0, 1000]`. It is data: it
/// serializes as that object.
///
/// # Examples
///
/// ```
/// use tungsten_kit::{Resist, Tags};
///
/// let resist = Resist::new().with_factor("damage", 0.5).with_factor("damage.fire", 0.5);
/// assert_eq!(resist.factor(&Tags::new(["damage.fire"])), 0.25, "the tag and its ancestor");
/// assert_eq!(resist.factor(&Tags::new(["damage.cold"])), 0.5);
/// assert_eq!(resist.factor(&Tags::new(["magic"])), 1.0, "no listed tag");
///
/// let immune = resist.with_factor("damage.poison", 0.0);
/// assert_eq!(immune.factor(&Tags::new(["damage.poison"])), 0.0);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "BTreeMap<String, f32>", into = "BTreeMap<String, f32>")]
pub struct Resist {
    /// Sorted by tag.
    factors: Vec<(String, f32)>,
}

impl TryFrom<BTreeMap<String, f32>> for Resist {
    type Error = String;

    fn try_from(factors: BTreeMap<String, f32>) -> Result<Self, Self::Error> {
        for (tag, &factor) in &factors {
            check_factor(tag, factor)?;
        }
        Ok(Self {
            factors: factors.into_iter().collect(),
        })
    }
}

impl From<Resist> for BTreeMap<String, f32> {
    fn from(resist: Resist) -> Self {
        resist.factors.into_iter().collect()
    }
}

/// `tag` a tag name and `factor` finite in `[0, 1000]`.
fn check_factor(tag: &str, factor: f32) -> Result<(), String> {
    if !valid_tag(tag) {
        return Err(format!(
            "`{tag}` is not a tag name (`[a-z][a-z0-9_]*` segments joined by dots)"
        ));
    }
    if !(factor.is_finite() && (0.0..=MAX_FACTOR).contains(&factor)) {
        return Err(format!(
            "{tag}: the factor must be finite and in [0, {MAX_FACTOR}] (got {factor})"
        ));
    }
    Ok(())
}

impl Resist {
    /// No resistances: every factor 1.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The resistances with `factor` for damage tagged `tag`, added or
    /// replaced; 0 is immunity.
    ///
    /// # Panics
    ///
    /// When `tag` is not a tag name, or `factor` is not finite and in
    /// `[0, 1000]`.
    #[must_use]
    #[track_caller]
    pub fn with_factor(mut self, tag: &str, factor: f32) -> Self {
        if let Err(error) = check_factor(tag, factor) {
            panic!("Resist::with_factor: {error}");
        }
        match self
            .factors
            .binary_search_by(|(known, _)| known.as_str().cmp(tag))
        {
            Ok(at) => self.factors[at].1 = factor,
            Err(at) => self.factors.insert(at, (tag.to_owned(), factor)),
        }
        self
    }

    /// The product of the factors whose tag `tags` carries, 1 when none
    /// does and 0 when one of them is 0, clamped to `f32::MAX`.
    #[must_use]
    pub fn factor(&self, tags: &Tags) -> f32 {
        self.product(tags)
            .map_or(0.0, |product| product.min(f64::from(f32::MAX)) as f32)
    }

    /// The product of the matching factors in `f64`, or `None` when one of
    /// them is 0: immunity, found before any product.
    pub(crate) fn product(&self, tags: &Tags) -> Option<f64> {
        let matching = || self.factors.iter().filter(|(tag, _)| tags.has(tag));
        if matching().any(|&(_, factor)| factor == 0.0) {
            return None;
        }
        Some(matching().map(|&(_, factor)| f64::from(factor)).product())
    }
}

/// A shield that takes damage before health: a hit's amount, after the
/// resistances, comes off the shield first and only the rest off the
/// [`Health`](crate::Health). After `delay` seconds of game time without a
/// hit it recharges at `recharge` per second, up to its max;
/// [`HealthPlugin`](crate::HealthPlugin)'s [`SHIELD_TICK`] counts both on
/// the fixed step. A hit the shield takes whole is
/// [`Hit::Absorbed`](crate::Hit::Absorbed). A heal never fills a shield;
/// [`restore`](Self::restore) does.
///
/// Scene name `shield`: `{ "max": 3, "recharge": 1, "delay": 2 }`,
/// `recharge` and `delay` 0 by default, so it does not recharge. It is
/// data: a game's save can hold it.
///
/// # Examples
///
/// ```
/// use tungsten_core::{Plugin, Schedule, Stage, Time, World};
/// use tungsten_kit::{Health, HealthPlugin, Hit, Shield, damage};
///
/// let mut world = World::new();
/// world.insert_resource(Time::new());
/// let mut schedule = Schedule::new();
/// HealthPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// let carrier = world.spawn_with((Health::new(5.0), Shield::new(2.0).with_recharge(1.0, 0.5)));
/// assert_eq!(damage(&mut world, carrier, 1.5), Hit::Absorbed);
/// assert_eq!(damage(&mut world, carrier, 1.5), Hit::Hurt, "0.5 left, then 1 off health");
/// assert_eq!(world.get::<Health>(carrier).unwrap().current(), 4.0);
///
/// // Half a second without a hit, then a second of recharge.
/// for _ in 0..90 {
///     world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
///     schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
/// }
/// let shield = world.get::<Shield>(carrier).unwrap();
/// assert!((shield.current() - 1.0).abs() < 1e-4, "{}", shield.current());
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ShieldFields", into = "ShieldFields")]
pub struct Shield {
    current: f32,
    max: f32,
    recharge: f32,
    delay: f32,
    /// Game seconds since its last hit.
    since_hit: f32,
}

/// A shield as a save writes it, before its values are checked.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ShieldFields {
    current: f32,
    max: f32,
    #[serde(default)]
    recharge: f32,
    #[serde(default)]
    delay: f32,
    #[serde(default)]
    since_hit: f32,
}

impl TryFrom<ShieldFields> for Shield {
    type Error = String;

    fn try_from(fields: ShieldFields) -> Result<Self, Self::Error> {
        let ShieldFields {
            current,
            max,
            recharge,
            delay,
            since_hit,
        } = fields;
        if !(max.is_finite() && max > 0.0) {
            return Err(format!("max: must be finite and > 0 (got {max})"));
        }
        if !(current.is_finite() && (0.0..=max).contains(&current)) {
            return Err(format!("current: must be within 0..={max} (got {current})"));
        }
        for (field, value) in [
            ("recharge", recharge),
            ("delay", delay),
            ("since_hit", since_hit),
        ] {
            if !(value.is_finite() && value >= 0.0) {
                return Err(format!("{field}: must be finite and >= 0 (got {value})"));
            }
        }
        Ok(Self {
            current,
            max,
            recharge,
            delay,
            since_hit,
        })
    }
}

impl From<Shield> for ShieldFields {
    fn from(shield: Shield) -> Self {
        Self {
            current: shield.current,
            max: shield.max,
            recharge: shield.recharge,
            delay: shield.delay,
            since_hit: shield.since_hit,
        }
    }
}

impl Shield {
    /// A full shield of `max`, which does not recharge.
    ///
    /// # Panics
    ///
    /// When `max` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn new(max: f32) -> Self {
        assert!(
            max.is_finite() && max > 0.0,
            "Shield::new: max must be finite and > 0 (got {max})"
        );
        Self {
            current: max,
            max,
            recharge: 0.0,
            delay: 0.0,
            since_hit: 0.0,
        }
    }

    /// The shield, recharging `per_second` once `delay` seconds of game time
    /// have passed since its last hit.
    ///
    /// # Panics
    ///
    /// When either is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_recharge(mut self, per_second: f32, delay: f32) -> Self {
        for (name, value) in [("per_second", per_second), ("delay", delay)] {
            assert!(
                value.is_finite() && value >= 0.0,
                "Shield::with_recharge: {name} must be finite and >= 0 (got {value})"
            );
        }
        self.recharge = per_second;
        self.delay = delay;
        self
    }

    /// The shield left, from 0 to [`max`](Self::max).
    #[must_use]
    pub fn current(&self) -> f32 {
        self.current
    }

    /// The full shield.
    #[must_use]
    pub fn max(&self) -> f32 {
        self.max
    }

    /// Adds `amount`, up to [`max`](Self::max): a pickup that fills the
    /// shield, which a heal does not.
    ///
    /// # Panics
    ///
    /// When `amount` is not finite and at least 0.
    #[track_caller]
    pub fn restore(&mut self, amount: f32) {
        assert!(
            amount.is_finite() && amount >= 0.0,
            "Shield::restore: amount must be finite and >= 0 (got {amount})"
        );
        self.current = (self.current + amount).min(self.max);
    }

    /// Takes up to `amount` and restarts the delay; what it took.
    pub(crate) fn take(&mut self, amount: f32) -> f32 {
        let taken = self.current.min(amount);
        self.current -= taken;
        self.since_hit = 0.0;
        taken
    }

    /// Counts `dt` of game time without a hit and recharges by the part of
    /// it past the delay, so a recharge lands the same at any frame rate.
    fn tick(&mut self, dt: f32) {
        let before = self.since_hit;
        self.since_hit = (before + dt).min(f32::MAX);
        if self.recharge <= 0.0 || self.current >= self.max {
            return;
        }
        let past = self.since_hit - self.delay.max(before);
        if past > 0.0 {
            self.current = (self.current + self.recharge * past.min(dt)).min(self.max);
        }
    }
}

/// [`SHIELD_TICK`]'s work: every shield counts the step.
pub(crate) fn shield_tick(world: &mut World) {
    let dt = world.get_resource::<Time>().map_or(0.0, Time::delta);
    if dt <= 0.0 {
        return;
    }
    for shield in world.query_mut::<&mut Shield>() {
        shield.tick(dt);
    }
}

/// `components.resist` as a scene writes it: factors by tag, in name
/// order, each checked.
#[derive(Serialize, Deserialize)]
#[serde(try_from = "BTreeMap<String, f32>")]
pub(crate) struct ResistDef(pub(crate) BTreeMap<String, f32>);

impl TryFrom<BTreeMap<String, f32>> for ResistDef {
    type Error = String;

    fn try_from(factors: BTreeMap<String, f32>) -> Result<Self, Self::Error> {
        for (tag, &factor) in &factors {
            check_factor(tag, factor)?;
        }
        Ok(Self(factors))
    }
}

impl From<ResistDef> for Resist {
    fn from(def: ResistDef) -> Self {
        Self {
            factors: def.0.into_iter().collect(),
        }
    }
}

/// `components.shield` as a scene writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ShieldDef {
    #[serde(deserialize_with = "crate::foundation::positive")]
    pub(crate) max: f32,
    #[serde(default, deserialize_with = "crate::foundation::non_negative")]
    pub(crate) recharge: f32,
    #[serde(default, deserialize_with = "crate::foundation::non_negative")]
    pub(crate) delay: f32,
}

/// The descriptions of `resist` and `shield`.
pub(crate) fn resist_description() -> ComponentDescription {
    ComponentDescription::new(
        RESIST,
        "Damage factors by damage tag: a request's matching factors multiply its amount, and a factor of 0 is immunity.",
    )
    .field(FieldDescription::new(
        "factors",
        FieldType::Object("damage tag → factor in [0, 1000]"),
        "the value itself: each tag declared, matching a request's tag or its dotted descendants",
    ))
    .genre(TOP_DOWN)
    .genre(SHMUP)
    .sample(r#"{ "damage.fire": 0.5 }"#)
    .roundtrip(round_trip::<ResistDef>)
}

pub(crate) fn shield_description() -> ComponentDescription {
    ComponentDescription::new(
        SHIELD,
        "Takes damage before health, and recharges after a delay without a hit.",
    )
    .field(FieldDescription::new(
        "max",
        FieldType::F32 {
            min: Bound::Exclusive(0.0),
            max: Bound::Open,
        },
        "the full shield, which it starts at",
    ))
    .field(
        FieldDescription::new(
            "recharge",
            FieldType::F32 {
                min: Bound::Inclusive(0.0),
                max: Bound::Open,
            },
            "shield restored per second of game time once the delay has passed",
        )
        .default("0.0"),
    )
    .field(
        FieldDescription::new(
            "delay",
            FieldType::F32 {
                min: Bound::Inclusive(0.0),
                max: Bound::Open,
            },
            "game seconds without a hit before it recharges",
        )
        .default("0.0"),
    )
    .genre(SHMUP)
    .genre(TOP_DOWN)
    .sample(r#"{ "max": 3 }"#)
    .roundtrip(round_trip::<ShieldDef>)
}

impl Inspectable for Resist {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        let factors: Vec<String> = self
            .factors
            .iter()
            .map(|(tag, factor)| format!("{tag} × {factor}"))
            .collect();
        vec![("resist", factors.join(", "))]
    }
}

impl Inspectable for Shield {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        vec![
            ("shield", format!("{} / {}", self.current, self.max)),
            (
                "recharge",
                format!("{} /s after {:.2} s", self.recharge, self.delay),
            ),
        ]
    }
}

#[cfg(test)]
#[path = "../tests/defense.rs"]
mod tests;
