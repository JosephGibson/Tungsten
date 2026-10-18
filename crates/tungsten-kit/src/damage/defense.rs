//! Defences between a damage request and health: resistances by damage
//! tag with modifiers, and a shield that takes damage first and recharges
//! after a delay.

use std::collections::BTreeMap;
use std::fmt;

use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use tungsten_core::{Inspectable, Time, World};

use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, SHMUP, TOP_DOWN, round_trip,
};
use crate::foundation::stats::{Modifier, modified};
use crate::foundation::tags::{Tags, valid_tag};

/// [`HealthPlugin`](crate::HealthPlugin)'s system that recharges shields, in
/// the tick phase.
pub const SHIELD_TICK: &str = "shield_tick";

/// The scene components the health plugin registers for the defences.
pub(crate) const RESIST: &str = "resist";
pub(crate) const SHIELD: &str = "shield";

/// The largest resistance factor.
const MAX_FACTOR: f32 = 1000.0;

/// An entity's resistances: a factor per damage tag, which multiplies each
/// part of a [`DamageRequest`](crate::DamageRequest) whose types carry that
/// tag, exactly or as a dotted ancestor: a factor for `damage` applies to a
/// `damage.fire` part too. A base factor of 0 is immunity, whatever the
/// modifiers: the part drops before any product. The matching factors
/// multiply in `f64` and the amount they make is clamped to `f32::MAX`, so
/// every amount stays finite; a factor in `(0, 1)` first gains the part's
/// [`Penetration`](crate::Penetration) points, capped at 1.
///
/// A factor takes modifiers by source, as a stat does
/// ([`add_modifier`](Self::add_modifier)): its value is `((base + Σadd) ×
/// (1 + Σpct) × Πmul)`, replaced by the last-added override, in `f64`,
/// clamped to `[0, 1000]`, the base 1 for a tag listed by a modifier only.
/// A curse that doubles fire damage is `Modifier::Mul(2.0)` on `fire`, from
/// `curse`, which [`remove_modifiers`](Self::remove_modifiers) takes back.
///
/// Scene name `resist`: `{ "damage.fire": 0.5, "damage.poison": 0 }`, each
/// key a declared tag, each base finite in `[0, 1000]`. It is data: a save
/// writes a factor without modifiers as its base, as the scene does, and
/// one with modifiers as `{ "base": 0.5, "modifiers": [["curse", { "mul": 2 }]] }`,
/// `base` left out for a tag listed by a modifier only.
///
/// # Examples
///
/// ```
/// use tungsten_kit::{Modifier, Resist, Tags};
///
/// let resist = Resist::new().with_factor("damage", 0.5).with_factor("damage.fire", 0.5);
/// assert_eq!(resist.factor(&Tags::new(["damage.fire"])), 0.25, "the tag and its ancestor");
/// assert_eq!(resist.factor(&Tags::new(["damage.cold"])), 0.5);
/// assert_eq!(resist.factor(&Tags::new(["magic"])), 1.0, "no listed tag");
///
/// let mut cursed = resist.with_factor("damage.poison", 0.0);
/// cursed.add_modifier("damage.fire", "curse", Modifier::Mul(3.0));
/// cursed.add_modifier("damage.poison", "curse", Modifier::Add(1.0));
/// assert_eq!(cursed.factor(&Tags::new(["damage.fire"])), 0.75);
/// assert_eq!(cursed.factor(&Tags::new(["damage.poison"])), 0.0, "a base of 0 stays immune");
/// assert_eq!(cursed.remove_modifiers("curse"), 2);
/// assert_eq!(cursed.base("damage.fire"), Some(0.5));
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ResistSave", into = "ResistSave")]
pub struct Resist {
    /// Sorted by tag.
    entries: Vec<Entry>,
}

/// One resisted tag: its base, its modifiers by source in add order, and
/// the factor they make.
#[derive(Debug, Clone, PartialEq)]
struct Entry {
    tag: String,
    /// `None` for a tag listed by a modifier only, whose base is 1.
    base: Option<f32>,
    modifiers: Vec<(String, Modifier)>,
    factor: f32,
}

impl Entry {
    fn new(tag: &str, base: Option<f32>, modifiers: Vec<(String, Modifier)>) -> Self {
        let mut entry = Self {
            tag: tag.to_owned(),
            base,
            modifiers,
            factor: 0.0,
        };
        entry.recompute();
        entry
    }

    /// A base of 0 stays 0; a base without modifiers is the factor as it
    /// is; otherwise the modifiers over the base, or over 1 without one.
    fn recompute(&mut self) {
        self.factor = match self.base {
            Some(base) if base == 0.0 || self.modifiers.is_empty() => base,
            base => modified(
                base.unwrap_or(1.0),
                self.modifiers.iter().map(|&(_, modifier)| modifier),
                0.0,
                MAX_FACTOR,
            ),
        };
    }
}

impl TryFrom<BTreeMap<String, f32>> for Resist {
    type Error = String;

    /// Resistances of these bases, without modifiers: the scene's form.
    fn try_from(factors: BTreeMap<String, f32>) -> Result<Self, Self::Error> {
        let mut entries = Vec::with_capacity(factors.len());
        for (tag, factor) in factors {
            check_factor(&tag, factor)?;
            entries.push(Entry::new(&tag, Some(factor), Vec::new()));
        }
        Ok(Self { entries })
    }
}

/// `tag` a tag name and `factor` finite in `[0, 1000]`.
fn check_factor(tag: &str, factor: f32) -> Result<(), String> {
    check_tag(tag)?;
    if !(factor.is_finite() && (0.0..=MAX_FACTOR).contains(&factor)) {
        return Err(format!(
            "{tag}: the factor must be finite and in [0, {MAX_FACTOR}] (got {factor})"
        ));
    }
    Ok(())
}

/// `tag` a tag name.
fn check_tag(tag: &str) -> Result<(), String> {
    if valid_tag(tag) {
        Ok(())
    } else {
        Err(format!(
            "`{tag}` is not a tag name (`[a-z][a-z0-9_]*` segments joined by dots)"
        ))
    }
}

/// Every modifier finite.
fn check_modifiers(tag: &str, modifiers: &[(String, Modifier)]) -> Result<(), String> {
    match modifiers.iter().find(|(_, modifier)| !modifier.is_finite()) {
        Some((_, modifier)) => Err(format!(
            "{tag}: a modifier must be finite (got {modifier:?})"
        )),
        None => Ok(()),
    }
}

impl Resist {
    /// No resistances: every factor 1.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The resistances with base `factor` for damage tagged `tag`, added or
    /// replaced, its modifiers kept; 0 is immunity.
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
        match self.search(tag) {
            Ok(at) => {
                self.entries[at].base = Some(factor);
                self.entries[at].recompute();
            }
            Err(at) => self
                .entries
                .insert(at, Entry::new(tag, Some(factor), Vec::new())),
        }
        self
    }

    /// Adds `modifier` to `tag`'s factor from `source`, after its earlier
    /// ones, as [`Stats::add_modifier`](crate::Stats::add_modifier) does; a
    /// tag without a factor takes one of base 1, which goes with its last
    /// modifier. A base of 0 stays immune whatever its modifiers.
    ///
    /// # Panics
    ///
    /// When `tag` is not a tag name or the modifier's value is not finite.
    #[track_caller]
    pub fn add_modifier(&mut self, tag: &str, source: &str, modifier: Modifier) {
        if let Err(error) = check_tag(tag) {
            panic!("Resist::add_modifier: {error}");
        }
        assert!(
            modifier.is_finite(),
            "Resist::add_modifier: {tag}: the modifier must be finite (got {modifier:?})"
        );
        match self.search(tag) {
            Ok(at) => {
                let entry = &mut self.entries[at];
                entry.modifiers.push((source.to_owned(), modifier));
                entry.recompute();
            }
            Err(at) => self.entries.insert(
                at,
                Entry::new(tag, None, vec![(source.to_owned(), modifier)]),
            ),
        }
    }

    /// Removes every modifier `source` gave, on every tag, and recomputes
    /// those factors, dropping a tag left with no base and no modifier; how
    /// many it removed.
    pub fn remove_modifiers(&mut self, source: &str) -> usize {
        let mut removed = 0;
        for entry in &mut self.entries {
            let before = entry.modifiers.len();
            entry.modifiers.retain(|(from, _)| from != source);
            if entry.modifiers.len() != before {
                removed += before - entry.modifiers.len();
                entry.recompute();
            }
        }
        self.entries
            .retain(|entry| entry.base.is_some() || !entry.modifiers.is_empty());
        removed
    }

    /// `tag`'s base factor, when it lists one.
    #[must_use]
    pub fn base(&self, tag: &str) -> Option<f32> {
        self.search(tag).ok().and_then(|at| self.entries[at].base)
    }

    /// The product of the factors whose tag `tags` carries, each after its
    /// modifiers, 1 when none does and 0 when one of them is 0, clamped to
    /// `f32::MAX`.
    #[must_use]
    pub fn factor(&self, tags: &Tags) -> f32 {
        self.product_with(tags, 0.0)
            .map_or(0.0, |product| product.min(f64::from(f32::MAX)) as f32)
    }

    /// The product of the matching factors in `f64`, each in `(0, 1)`
    /// raised by `points` and capped at 1, or `None` when one of them is 0:
    /// an immunity or a modifier's 0, found before any product.
    pub(crate) fn product_with(&self, tags: &Tags, points: f32) -> Option<f64> {
        let matching = || self.entries.iter().filter(|entry| tags.has(&entry.tag));
        if matching().any(|entry| entry.factor == 0.0) {
            return None;
        }
        Some(
            matching()
                .map(|entry| {
                    let factor = f64::from(entry.factor);
                    if points > 0.0 && entry.factor < 1.0 {
                        (factor + f64::from(points)).min(1.0)
                    } else {
                        factor
                    }
                })
                .product(),
        )
    }

    fn search(&self, tag: &str) -> Result<usize, usize> {
        self.entries
            .binary_search_by(|entry| entry.tag.as_str().cmp(tag))
    }
}

/// A resistance as a save writes it: factors by tag, each a bare base or a
/// base with its modifiers.
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct ResistSave(BTreeMap<String, SavedFactor>);

/// One saved factor: `0.5`, or `{ "base": 0.5, "modifiers": [..] }` with
/// `base` optional, never `{}`.
struct SavedFactor {
    base: Option<f32>,
    modifiers: Vec<(String, Modifier)>,
}

impl Serialize for SavedFactor {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct WithModifiers<'a> {
            #[serde(skip_serializing_if = "Option::is_none")]
            base: Option<f32>,
            modifiers: &'a [(String, Modifier)],
        }
        match (self.base, self.modifiers.is_empty()) {
            (Some(base), true) => serializer.serialize_f32(base),
            _ => WithModifiers {
                base: self.base,
                modifiers: &self.modifiers,
            }
            .serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for SavedFactor {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(SavedFactorVisitor)
    }
}

/// Reads either form of a saved factor.
struct SavedFactorVisitor;

impl SavedFactorVisitor {
    fn base(value: f64) -> SavedFactor {
        SavedFactor {
            base: Some(value as f32),
            modifiers: Vec::new(),
        }
    }
}

impl<'de> Visitor<'de> for SavedFactorVisitor {
    type Value = SavedFactor;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a factor, or an object of `base` and `modifiers`")
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<SavedFactor, E> {
        Ok(Self::base(value))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<SavedFactor, E> {
        Ok(Self::base(value as f64))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<SavedFactor, E> {
        Ok(Self::base(value as f64))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<SavedFactor, A::Error> {
        let mut base = None;
        let mut modifiers = None;
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "base" => {
                    if base.is_some() {
                        return Err(de::Error::duplicate_field("base"));
                    }
                    base = Some(map.next_value::<f32>()?);
                }
                "modifiers" => {
                    if modifiers.is_some() {
                        return Err(de::Error::duplicate_field("modifiers"));
                    }
                    modifiers = Some(map.next_value::<Vec<(String, Modifier)>>()?);
                }
                other => return Err(de::Error::unknown_field(other, &["base", "modifiers"])),
            }
        }
        let modifiers = modifiers.unwrap_or_default();
        if base.is_none() && modifiers.is_empty() {
            return Err(de::Error::custom(
                "a factor names its `base`, its `modifiers` or both",
            ));
        }
        Ok(SavedFactor { base, modifiers })
    }
}

impl TryFrom<ResistSave> for Resist {
    type Error = String;

    fn try_from(save: ResistSave) -> Result<Self, Self::Error> {
        let mut entries = Vec::with_capacity(save.0.len());
        for (tag, factor) in save.0 {
            match factor.base {
                Some(base) => check_factor(&tag, base)?,
                None => check_tag(&tag)?,
            }
            check_modifiers(&tag, &factor.modifiers)?;
            entries.push(Entry::new(&tag, factor.base, factor.modifiers));
        }
        Ok(Self { entries })
    }
}

impl From<Resist> for ResistSave {
    fn from(resist: Resist) -> Self {
        Self(
            resist
                .entries
                .into_iter()
                .map(|entry| {
                    let factor = SavedFactor {
                        base: entry.base,
                        modifiers: entry.modifiers,
                    };
                    (entry.tag, factor)
                })
                .collect(),
        )
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
            entries: def
                .0
                .into_iter()
                .map(|(tag, factor)| Entry::new(&tag, Some(factor), Vec::new()))
                .collect(),
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
        "Damage factors by damage tag: a hit part's matching factors multiply its amount, and a factor of 0 is immunity.",
    )
    .field(FieldDescription::new(
        "factors",
        FieldType::Object("damage tag → factor in [0, 1000]"),
        "the value itself: each tag declared, matching a part's type or its dotted descendants; a save adds each factor's modifiers",
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
    /// Each tag's factor, with its base and modifiers when it has any.
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        let factors: Vec<String> = self
            .entries
            .iter()
            .map(|entry| {
                if entry.modifiers.is_empty() {
                    return format!("{} × {}", entry.tag, entry.factor);
                }
                let base = entry
                    .base
                    .map_or_else(|| "none".to_string(), |base| base.to_string());
                let modifiers: Vec<String> = entry
                    .modifiers
                    .iter()
                    .map(|(source, modifier)| format!("{source} {modifier:?}"))
                    .collect();
                format!(
                    "{} × {} (base {base}; {})",
                    entry.tag,
                    entry.factor,
                    modifiers.join(", ")
                )
            })
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
