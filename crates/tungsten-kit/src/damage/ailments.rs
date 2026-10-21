//! Ailments: a ladder of rungs on a damage type that a target's bucket of
//! that type climbs, each rung an effect gated by a fraction of the
//! target's threshold, the tags it must carry and a chance; the buckets
//! drain on the tick.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tungsten_core::{
    DataRef, Entity, EventCursor, EventQueue, Inspectable, MissingAsset, Schedule, SpawnContext,
    Time, World, system,
};

use crate::damage::effects::{EFFECT_TICK, Effect, EffectRequest, definition};
use crate::damage::health::{DamagedPart, Health};
use crate::damage::kinds::{ladder, ladder_of};
use crate::damage::pipeline::DAMAGE_RESOLVE;
use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, SHMUP, TOP_DOWN, describe,
    describe_shape, round_trip,
};
use crate::foundation::inspect::register_rows;
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::rng::KitRng;
use crate::foundation::stats::{STATS_APPLY, Stats};
use crate::foundation::tags::{Tags, check_declared, valid_tag};
use crate::foundation::trace::{TraceKind, trace};
use crate::foundation::{non_negative, optional_positive};

/// [`EffectsPlugin`](crate::EffectsPlugin)'s system that feeds the buckets
/// from the step's landed parts and climbs their ladders, in the resolve
/// phase after [`DAMAGE_RESOLVE`](crate::DAMAGE_RESOLVE) and before
/// [`EFFECT_RESOLVE`](crate::EFFECT_RESOLVE).
pub const AILMENT_FEED: &str = "ailment_feed";

/// [`EffectsPlugin`](crate::EffectsPlugin)'s system that drains the
/// buckets, in the tick phase after [`STATS_APPLY`](crate::STATS_APPLY)
/// when it is registered.
pub const AILMENT_TICK: &str = "ailment_tick";

/// The scene component, and the shape of a ladder's rung.
const AILMENT_THRESHOLD: &str = "ailment_threshold";
pub(crate) const AILMENT_RUNG: &str = "ailment_rung";

/// The stat that scales a threshold when the entity has it: a game stat,
/// declared in its `stats` kind, not well-known.
const AILMENT_THRESHOLD_MULT: &str = "ailment_threshold_mult";

/// One rung of a damage type's ladder: the effect it applies when the
/// bucket's fill reaches `needs` of the target's threshold, the target
/// carries every `requires` tag and none of its `blocked_by` tags, and the
/// `chance` draws. A ladder is written from the lowest rung up, and the
/// feed climbs it from the top rung down, so a full bucket takes the
/// highest rung the target can reach. A rung skipped by a `blocked_by` tag
/// leaves the bucket filling for the rungs above: a chill that holds while
/// the cold builds to a freeze.
///
/// Data form, in a `damage_types` definition's `ailments` list:
/// `{ "effect": "chilled", "needs": 0.5, "requires": [], "blocked_by": [], "chance": 1, "window": 4 }`,
/// `needs` a fraction above 0 or left out, `requires` and `blocked_by`
/// declared tags, `chance` in `(0, 1]` (1 by default) and `window` seconds
/// above 0, required with `needs`: the first `needs` rung's window is what
/// the bucket drains over.
///
/// # Examples
///
/// ```
/// use tungsten_core::DataRef;
/// use tungsten_kit::AilmentRung;
///
/// let frozen = AilmentRung::new(DataRef::by_id("frozen"))
///     .with_needs(1.0, 4.0)
///     .with_requires(["status.chilled"])
///     .with_chance(0.75);
/// assert_eq!((frozen.needs(), frozen.window(), frozen.chance()), (Some(1.0), Some(4.0), 0.75));
/// assert_eq!(frozen.requires(), ["status.chilled"]);
/// // Skipped while chilled, so the cold climbs on to `frozen`.
/// let chilled = AilmentRung::new(DataRef::by_id("chilled"))
///     .with_needs(0.3, 4.0)
///     .with_blocked_by(["status.chilled"]);
/// assert_eq!(chilled.blocked_by(), ["status.chilled"]);
/// let read: AilmentRung = serde_json::from_str(r#"{ "effect": "chilled", "needs": 0.5, "window": 4 }"#).unwrap();
/// assert_eq!(read.needs(), Some(0.5));
/// assert!(serde_json::from_str::<AilmentRung>(r#"{ "effect": "chilled", "needs": 0.5 }"#).is_err(), "needs a window");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "RungFields", into = "RungFields")]
pub struct AilmentRung {
    effect: DataRef<Effect>,
    needs: Option<f32>,
    requires: Vec<String>,
    blocked_by: Vec<String>,
    chance: f32,
    window: Option<f32>,
}

/// A rung as data writes it, before its values are checked together.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RungFields {
    effect: DataRef<Effect>,
    #[serde(default, deserialize_with = "optional_positive")]
    needs: Option<f32>,
    #[serde(default)]
    requires: Vec<String>,
    #[serde(default)]
    blocked_by: Vec<String>,
    #[serde(default = "full_chance")]
    chance: f32,
    #[serde(default, deserialize_with = "optional_positive")]
    window: Option<f32>,
}

fn full_chance() -> f32 {
    1.0
}

impl TryFrom<RungFields> for AilmentRung {
    type Error = String;

    fn try_from(fields: RungFields) -> Result<Self, Self::Error> {
        if !(fields.chance.is_finite() && fields.chance > 0.0 && fields.chance <= 1.0) {
            return Err(format!("chance: must be in (0, 1] (got {})", fields.chance));
        }
        if fields.needs.is_some() && fields.window.is_none() {
            return Err("window: required with `needs`".to_string());
        }
        for (field, tags) in [
            ("requires", &fields.requires),
            ("blocked_by", &fields.blocked_by),
        ] {
            if let Some(tag) = tags.iter().find(|tag| !valid_tag(tag)) {
                return Err(format!(
                    "{field}: `{tag}` is not a tag name (`[a-z][a-z0-9_]*` segments joined by dots)"
                ));
            }
        }
        Ok(Self {
            effect: fields.effect,
            needs: fields.needs,
            requires: fields.requires,
            blocked_by: fields.blocked_by,
            chance: fields.chance,
            window: fields.window,
        })
    }
}

impl From<AilmentRung> for RungFields {
    fn from(rung: AilmentRung) -> Self {
        Self {
            effect: rung.effect,
            needs: rung.needs,
            requires: rung.requires,
            blocked_by: rung.blocked_by,
            chance: rung.chance,
            window: rung.window,
        }
    }
}

impl AilmentRung {
    /// A rung applying `effect` whatever the fill, to any target, always.
    #[must_use]
    pub fn new(effect: DataRef<Effect>) -> Self {
        Self {
            effect,
            needs: None,
            requires: Vec::new(),
            blocked_by: Vec::new(),
            chance: 1.0,
            window: None,
        }
    }

    /// The rung, reached when the fill is at least `fraction` of the
    /// threshold, the bucket draining over `window` seconds.
    ///
    /// # Panics
    ///
    /// When either is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_needs(mut self, fraction: f32, window: f32) -> Self {
        assert!(
            fraction.is_finite() && fraction > 0.0 && window.is_finite() && window > 0.0,
            "AilmentRung::with_needs: fraction and window must be finite and > 0 (got {fraction}, {window})"
        );
        self.needs = Some(fraction);
        self.window = Some(window);
        self
    }

    /// The rung, for a target carrying every one of `tags`.
    ///
    /// # Panics
    ///
    /// When a name is not a tag name.
    #[must_use]
    #[track_caller]
    pub fn with_requires(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        for tag in tags {
            let tag = tag.into();
            assert!(
                valid_tag(&tag),
                "AilmentRung::with_requires: `{tag}` is not a tag name (`[a-z][a-z0-9_]*` segments joined by dots)"
            );
            self.requires.push(tag);
        }
        self
    }

    /// The rung, skipped while the target carries any of `tags` or the
    /// step's feed has granted one to it already: a skipped rung leaves the
    /// bucket filling for the rungs above, as a chill that holds while the
    /// cold builds to a freeze. Each tag is checked as declared at load.
    ///
    /// # Panics
    ///
    /// When a name is not a tag name.
    #[must_use]
    #[track_caller]
    pub fn with_blocked_by(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        for tag in tags {
            let tag = tag.into();
            assert!(
                valid_tag(&tag),
                "AilmentRung::with_blocked_by: `{tag}` is not a tag name (`[a-z][a-z0-9_]*` segments joined by dots)"
            );
            self.blocked_by.push(tag);
        }
        self
    }

    /// The rung, taken with probability `chance` once reached.
    ///
    /// # Panics
    ///
    /// When `chance` is not in `(0, 1]`.
    #[must_use]
    #[track_caller]
    pub fn with_chance(mut self, chance: f32) -> Self {
        assert!(
            chance.is_finite() && chance > 0.0 && chance <= 1.0,
            "AilmentRung::with_chance: chance must be in (0, 1] (got {chance})"
        );
        self.chance = chance;
        self
    }

    /// The effect it applies.
    #[must_use]
    pub fn effect(&self) -> &DataRef<Effect> {
        &self.effect
    }

    /// The fraction of the threshold it needs, when it needs one.
    #[must_use]
    pub fn needs(&self) -> Option<f32> {
        self.needs
    }

    /// The tags the target must carry.
    #[must_use]
    pub fn requires(&self) -> &[String] {
        &self.requires
    }

    /// The tags that skip the rung.
    #[must_use]
    pub fn blocked_by(&self) -> &[String] {
        &self.blocked_by
    }

    /// Its chance once reached.
    #[must_use]
    pub fn chance(&self) -> f32 {
        self.chance
    }

    /// The seconds the bucket drains over, when it has `needs`.
    #[must_use]
    pub fn window(&self) -> Option<f32> {
        self.window
    }

    /// The load's check: the effect and the tags, under `field`.
    pub(crate) fn check(&self, world: &World, field: &str) -> Result<(), MissingAsset> {
        self.effect.check(world, &format!("{field}.effect"))?;
        for (i, tag) in self.requires.iter().enumerate() {
            check_declared(world, &format!("{field}.requires.{i}"), tag)?;
        }
        for (i, tag) in self.blocked_by.iter().enumerate() {
            check_declared(world, &format!("{field}.blocked_by.{i}"), tag)?;
        }
        Ok(())
    }
}

/// How much a target's buckets must hold before its ladders' `needs` rungs
/// reach: `(flat + of_max_health × Health::max) × ailment_threshold_mult`,
/// the stat read when the entity has it and floored at 0, 1 without it. A
/// threshold of 0, or an entity without one, leaves the `needs` rungs
/// unreached; rungs without `needs` apply on any hit.
///
/// Scene name `ailment_threshold`: `{ "flat": 20, "of_max_health": 0.1 }`,
/// both at least 0 and 0 by default. It is data.
///
/// # Examples
///
/// ```
/// use tungsten_kit::AilmentThreshold;
///
/// let golem = AilmentThreshold::new(20.0).with_of_max_health(0.1);
/// assert_eq!((golem.flat(), golem.of_max_health()), (20.0, 0.1));
/// let read: AilmentThreshold = serde_json::from_str(r#"{ "flat": 20 }"#).unwrap();
/// assert_eq!(read, AilmentThreshold::new(20.0));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AilmentThreshold {
    #[serde(default, deserialize_with = "non_negative")]
    flat: f32,
    #[serde(default, deserialize_with = "non_negative")]
    of_max_health: f32,
}

impl AilmentThreshold {
    /// A threshold of `flat`.
    ///
    /// # Panics
    ///
    /// When `flat` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn new(flat: f32) -> Self {
        assert!(
            flat.is_finite() && flat >= 0.0,
            "AilmentThreshold::new: flat must be finite and >= 0 (got {flat})"
        );
        Self {
            flat,
            of_max_health: 0.0,
        }
    }

    /// The threshold, plus `fraction` of the entity's full health.
    ///
    /// # Panics
    ///
    /// When `fraction` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_of_max_health(mut self, fraction: f32) -> Self {
        assert!(
            fraction.is_finite() && fraction >= 0.0,
            "AilmentThreshold::with_of_max_health: fraction must be finite and >= 0 (got {fraction})"
        );
        self.of_max_health = fraction;
        self
    }

    /// The flat part.
    #[must_use]
    pub fn flat(&self) -> f32 {
        self.flat
    }

    /// The fraction of the full health added.
    #[must_use]
    pub fn of_max_health(&self) -> f32 {
        self.of_max_health
    }
}

/// The threshold of `entity` now: 0 without the component.
fn threshold_of(world: &World, entity: Entity) -> f32 {
    let Some(threshold) = world.get::<AilmentThreshold>(entity) else {
        return 0.0;
    };
    let max = world.get::<Health>(entity).map_or(0.0, Health::max);
    let mult = world
        .get::<Stats>(entity)
        .and_then(|stats| stats.get(AILMENT_THRESHOLD_MULT))
        .map_or(1.0, |mult| mult.max(0.0));
    let value = (f64::from(threshold.flat) + f64::from(threshold.of_max_health) * f64::from(max))
        * f64::from(mult);
    value.clamp(0.0, f64::from(f32::MAX)) as f32
}

/// An entity's ailment buckets: the fill of each damage type's ladder it
/// has taken, by the type the ladder sits on, which the feed inserts and
/// fills and the tick drains. It is data, saved as `{ "elemental.cold": 12.5 }`.
///
/// # Examples
///
/// ```
/// use tungsten_kit::AilmentBuckets;
///
/// let buckets: AilmentBuckets = serde_json::from_str(r#"{ "elemental.cold": 12.5 }"#).unwrap();
/// assert_eq!(buckets.fill("elemental.cold"), 12.5);
/// assert_eq!(buckets.fill("toxic"), 0.0);
/// assert_eq!(buckets.iter().count(), 1);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "BTreeMap<String, f32>", into = "BTreeMap<String, f32>")]
pub struct AilmentBuckets {
    fills: BTreeMap<String, f32>,
}

impl TryFrom<BTreeMap<String, f32>> for AilmentBuckets {
    type Error = String;

    fn try_from(fills: BTreeMap<String, f32>) -> Result<Self, Self::Error> {
        for (name, &fill) in &fills {
            if !valid_tag(name) {
                return Err(format!(
                    "`{name}` is not a damage type (`[a-z][a-z0-9_]*` segments joined by dots)"
                ));
            }
            if !(fill.is_finite() && fill > 0.0) {
                return Err(format!("{name}: must be finite and > 0 (got {fill})"));
            }
        }
        Ok(Self { fills })
    }
}

impl From<AilmentBuckets> for BTreeMap<String, f32> {
    fn from(buckets: AilmentBuckets) -> Self {
        buckets.fills
    }
}

impl AilmentBuckets {
    /// Empty buckets.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The fill of the type `name`'s bucket; 0 when empty.
    #[must_use]
    pub fn fill(&self, name: &str) -> f32 {
        self.fills.get(name).copied().unwrap_or(0.0)
    }

    /// Each bucket's type and fill, by name.
    pub fn iter(&self) -> impl Iterator<Item = (&str, f32)> {
        self.fills.iter().map(|(name, fill)| (name.as_str(), *fill))
    }

    /// Adds `amount` to `name`'s bucket; the new fill.
    fn add(&mut self, name: &str, amount: f32) -> f32 {
        let fill = self.fills.entry(name.to_owned()).or_insert(0.0);
        *fill = (f64::from(*fill) + f64::from(amount)).min(f64::from(f32::MAX)) as f32;
        *fill
    }

    /// Empties `name`'s bucket.
    fn empty(&mut self, name: &str) {
        self.fills.remove(name);
    }

    /// Takes `amount` off `name`'s bucket, which leaves at 0.
    fn drain(&mut self, name: &str, amount: f32) {
        if let Some(fill) = self.fills.get_mut(name) {
            *fill -= amount;
            if *fill <= 0.0 {
                self.fills.remove(name);
            }
        }
    }
}

/// `components.ailment_threshold` as a scene writes it: the component.
type ThresholdDef = AilmentThreshold;

/// [`AILMENT_FEED`]'s state between steps: its place in the parts queue,
/// the step's parts, and the tags of the effects queued this run per
/// target, counted as carried.
#[derive(Default)]
struct Feeder {
    cursor: EventCursor<DamagedPart>,
    parts: Vec<DamagedPart>,
    pending: Vec<(Entity, String)>,
}

impl Feeder {
    fn run(&mut self, world: &mut World) {
        self.parts.clear();
        if let Some(queue) = world.get_resource::<EventQueue<DamagedPart>>() {
            self.parts.extend(self.cursor.read(queue).copied());
        }
        if self.parts.is_empty() {
            return;
        }
        self.pending.clear();
        let parts = std::mem::take(&mut self.parts);
        for part in &parts {
            self.feed(world, part);
        }
        self.parts = parts;
        self.pending.clear();
    }

    /// One part: its ladder's bucket filled, then the rungs from the top
    /// down, the first that passes queued and the bucket emptied.
    fn feed(&mut self, world: &mut World, part: &DamagedPart) {
        if part.periodic || part.kinds.is_empty() || !world.is_alive(part.entity) {
            return;
        }
        let target = part.entity;
        let Some((name, rungs)) = ladder(world, part.kinds) else {
            return;
        };
        let name = name.to_owned();
        let rungs = rungs.len();
        if !world.has::<AilmentBuckets>(target) {
            world.insert(target, AilmentBuckets::new());
        }
        let fill = world
            .get_mut::<AilmentBuckets>(target)
            .unwrap()
            .add(&name, part.amount);
        let threshold = threshold_of(world, target);
        for i in (0..rungs).rev() {
            let Some((effect, chance)) =
                self.reaches(world, target, part.kinds, i, fill, threshold)
            else {
                continue;
            };
            if chance < 1.0 {
                let mut rng = world.remove_resource::<KitRng>().unwrap_or_default();
                let roll = rng.unit();
                world.insert_resource(rng);
                if roll >= chance {
                    continue;
                }
            }
            let scale = if threshold > 0.0 {
                fill / threshold
            } else {
                1.0
            };
            let mut request = EffectRequest::new(target, effect.clone()).with_scale(scale);
            if let Some(source) = part.source {
                request = request.with_source(source);
            }
            if let Some(effect) = definition(world, &effect) {
                for tag in effect.grant() {
                    self.pending.push((target, tag.clone()));
                }
            }
            request.queue(world);
            world
                .get_mut::<AilmentBuckets>(target)
                .unwrap()
                .empty(&name);
            trace(world, target, || TraceKind::Event {
                name: "ailment",
                other: part.source,
                amount: Some(fill),
            });
            return;
        }
    }

    /// Whether rung `index` of the ladder a part of `kinds` feeds is
    /// reached by `target` at `fill` of `threshold`, carrying every
    /// `requires` tag and no `blocked_by` one, the tags this run queued for
    /// it counted: its effect and chance when it is.
    fn reaches(
        &self,
        world: &World,
        target: Entity,
        kinds: crate::DamageKinds,
        index: usize,
        fill: f32,
        threshold: f32,
    ) -> Option<(DataRef<Effect>, f32)> {
        let (_, rungs) = ladder(world, kinds)?;
        let rung = rungs.get(index)?;
        if let Some(needs) = rung.needs
            && !(threshold > 0.0 && f64::from(fill) >= f64::from(needs) * f64::from(threshold))
        {
            return None;
        }
        let carried = world.get::<Tags>(target);
        let has = |tag: &String| {
            carried.is_some_and(|tags| tags.has(tag))
                || self
                    .pending
                    .iter()
                    .any(|(entity, pending)| *entity == target && grants(pending, tag))
        };
        if !rung.requires.iter().all(has) || rung.blocked_by.iter().any(has) {
            return None;
        }
        Some((rung.effect.clone(), rung.chance))
    }
}

/// Whether a tag the feed queued, `name`, is `tag` or under it, as
/// [`Tags::has`] matches a carried one: a pending `status.chilled` meets a
/// rung's `status`.
fn grants(name: &str, tag: &str) -> bool {
    name == tag || (name.starts_with(tag) && name.as_bytes().get(tag.len()) == Some(&b'.'))
}

/// [`AILMENT_TICK`]'s state between steps.
#[derive(Default)]
struct Drainer {
    entities: Vec<Entity>,
    names: Vec<String>,
}

impl Drainer {
    /// Each bucket loses `threshold / window` a second, the window the
    /// ladder's first `needs` rung's; an empty bucket leaves the map.
    fn run(&mut self, world: &mut World) {
        let dt = world
            .get_resource::<Time>()
            .map_or(0.0, Time::delta)
            .max(0.0);
        if dt <= 0.0 {
            return;
        }
        self.entities.clear();
        self.entities.extend(
            world
                .query::<(Entity, &AilmentBuckets)>()
                .map(|(entity, _)| entity),
        );
        self.entities.sort_by_key(|entity| entity.id());
        let entities = std::mem::take(&mut self.entities);
        for &entity in &entities {
            let threshold = threshold_of(world, entity);
            if threshold <= 0.0 {
                continue;
            }
            self.names.clear();
            if let Some(buckets) = world.get::<AilmentBuckets>(entity) {
                self.names.extend(buckets.fills.keys().cloned());
            }
            let names = std::mem::take(&mut self.names);
            for name in &names {
                let window = ladder_of(world, name).and_then(|rungs| {
                    rungs
                        .iter()
                        .find(|rung| rung.needs.is_some())
                        .and_then(|rung| rung.window)
                });
                let Some(window) = window else {
                    continue;
                };
                let drain = (f64::from(threshold) / f64::from(window) * f64::from(dt)) as f32;
                if let Some(buckets) = world.get_mut::<AilmentBuckets>(entity) {
                    buckets.drain(name, drain);
                }
            }
            self.names = names;
        }
        self.entities = entities;
    }
}

/// Registers the `ailment_threshold` scene component, the buckets' rows,
/// the rung's description, [`AILMENT_FEED`] in the resolve phase after
/// [`DAMAGE_RESOLVE`](crate::DAMAGE_RESOLVE) and [`AILMENT_TICK`] in the
/// tick phase after [`STATS_APPLY`](crate::STATS_APPLY) and
/// [`EFFECT_TICK`](crate::EFFECT_TICK), each when registered:
/// [`EffectsPlugin`](crate::EffectsPlugin)'s part of the build for the
/// ladder.
pub(crate) fn register_ailments(schedule: &mut Schedule, world: &mut World) {
    ensure_phases(schedule);
    register_rows::<AilmentThreshold>(world, "AilmentThreshold");
    register_rows::<AilmentBuckets>(world, "AilmentBuckets");
    describe(
        world,
        ComponentDescription::new(
            AILMENT_THRESHOLD,
            "How much the entity's ailment buckets must hold before a ladder's `needs` rungs reach: flat plus a fraction of its full health, times its `ailment_threshold_mult` stat when it has one.",
        )
        .field(
            FieldDescription::new(
                "flat",
                FieldType::F32 {
                    min: Bound::Inclusive(0.0),
                    max: Bound::Open,
                },
                "the flat part",
            )
            .default("0.0"),
        )
        .field(
            FieldDescription::new(
                "of_max_health",
                FieldType::F32 {
                    min: Bound::Inclusive(0.0),
                    max: Bound::Open,
                },
                "a fraction of the entity's `health` max, added",
            )
            .default("0.0"),
        )
        .genre(TOP_DOWN)
        .genre(SHMUP)
        .sample("{}")
        .roundtrip(round_trip::<ThresholdDef>),
    );
    describe_shape(
        world,
        ComponentDescription::new(
            AILMENT_RUNG,
            "One rung of a damage type's ladder, written from the lowest up and climbed from the top down.",
        )
        .field(FieldDescription::new(
            "effect",
            FieldType::Data(crate::damage::effects::EFFECTS),
            "applied to the target from the part's source, scaled by the fill over the threshold; the load checks it",
        ))
        .field(
            FieldDescription::new(
                "needs",
                FieldType::F32 {
                    min: Bound::Exclusive(0.0),
                    max: Bound::Open,
                },
                "the fraction of the target's threshold the bucket must hold; any fill when left out",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "requires",
                FieldType::List(Box::new(FieldType::Tag)),
                "tags the target must carry, the tags of effects the feed queued for it this step counted; the load checks them",
            )
            .default("[]"),
        )
        .field(
            FieldDescription::new(
                "blocked_by",
                FieldType::List(Box::new(FieldType::Tag)),
                "tags that skip the rung while the target carries one, the tags of effects the feed queued for it this step counted, so the bucket fills on for the rungs above; the load checks them",
            )
            .default("[]"),
        )
        .field(
            FieldDescription::new(
                "chance",
                FieldType::F32 {
                    min: Bound::Exclusive(0.0),
                    max: Bound::Inclusive(1.0),
                },
                "drawn from the kit's random stream once the rung is reached",
            )
            .default("1.0"),
        )
        .field(
            FieldDescription::new(
                "window",
                FieldType::F32 {
                    min: Bound::Exclusive(0.0),
                    max: Bound::Open,
                },
                "seconds the bucket drains a threshold over, the first `needs` rung's; required with `needs`",
            )
            .default("null"),
        )
        .sample(r#"{ "effect": "chilled", "window": 4 }"#)
        .roundtrip(round_trip::<AilmentRung>),
    );
    world.register_component_with(
        AILMENT_THRESHOLD,
        |def: ThresholdDef, _: &SpawnContext<'_>| (def,),
    );
    let mut feeder = Feeder::default();
    schedule.add(
        Phase::Resolve.stage(),
        Phase::Resolve
            .join(system(AILMENT_FEED, move |world: &mut World| {
                feeder.run(world);
            }))
            .after_if_present(DAMAGE_RESOLVE),
    );
    let mut drainer = Drainer::default();
    schedule.add(
        Phase::Tick.stage(),
        Phase::Tick
            .join(system(AILMENT_TICK, move |world: &mut World| {
                drainer.run(world);
            }))
            .after_if_present(STATS_APPLY)
            .after_if_present(EFFECT_TICK),
    );
}

impl Inspectable for AilmentThreshold {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        vec![(
            "ailment threshold",
            format!("{} + {} of max health", self.flat, self.of_max_health),
        )]
    }
}

impl Inspectable for AilmentBuckets {
    /// One row per bucket.
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        self.fills
            .iter()
            .map(|(name, fill)| ("bucket", format!("{name} {fill:.2}")))
            .collect()
    }
}

#[cfg(test)]
#[path = "../tests/ailments.rs"]
mod tests;
