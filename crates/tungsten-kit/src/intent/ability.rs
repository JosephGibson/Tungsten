//! Abilities: what an entity can do on a trigger, at a cost, with a
//! cooldown and charges, at a target: a spawn, an effect, an impulse or a
//! hitbox window; the `abilities` data kind, the `abilities` scene
//! component and [`use_ability`].

use std::fmt::Write as _;

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::physics::Position;
use tungsten_core::{
    CommandBuffer, DataRef, DataRegistry, Entity, EventQueue, Inspectable, MissingAsset, Plugin,
    PrefabRegistry, Schedule, SpawnContext, Time, Transform, World, system,
};

use crate::count_down;
use crate::damage::effects::{EFFECTS, Effect, EffectRequest};
use crate::damage::health::Health;
use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, SHMUP, TOP_DOWN, describe,
    describe_kind, describe_shape, round_trip,
};
use crate::foundation::inspect::register_rows;
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::rng::KitRng;
use crate::foundation::stats::Stats;
use crate::foundation::tags::{TagFilter, Tags, check_declared, valid_tag};
use crate::foundation::trace::{TraceKind, trace};
use crate::foundation::{non_negative, positive};
use crate::intent::action_input::{ACTION_INPUT, Intent};
use crate::intent::aim::{AIM, Aim};
use crate::lifecycle::ended::Despawning;
use crate::motion::drive::point_of;
use crate::motion::knockback::knock_back;
use crate::motion::linear::LinearMotion;
use crate::motion::target::describe_target;
use crate::overlap::hit::Hitbox;

/// [`AbilitiesPlugin`]'s system that counts cooldowns, refills and
/// intervals and fires the triggers, in the intent phase after
/// [`ACTION_INPUT`](crate::ACTION_INPUT) and [`AIM`](crate::AIM) when they
/// are registered.
pub const ABILITY_TICK: &str = "ability_tick";

/// The data kind of abilities, and the scene component.
pub(crate) const ABILITIES: &str = "abilities";
/// The shapes an ability's fields refer to.
const ABILITY: &str = "ability";
const ABILITY_TRIGGER: &str = "ability_trigger";
const ABILITY_COST: &str = "ability_cost";
const ABILITY_CHARGES: &str = "ability_charges";
const ABILITY_TARGET: &str = "ability_target";
const ABILITY_PAYLOAD: &str = "ability_payload";
const SPAWN_PAYLOAD: &str = "spawn_payload";
/// The target's `nearest` shape, which [`describe_target`] describes.
const NEAREST_SHAPE: &str = "nearest";
/// The scene component the spawn payload's prefab must hold with a speed.
const LINEAR_MOTION: &str = "linear_motion";

/// Whether `name` is a stat name, as `Stats` reads one.
fn valid_stat(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// What fires an ability. A match outside this crate needs a wildcard arm,
/// since later forms are additions.
///
/// Data form, one field: `{ "action": "attack" }`, an [`Intent`] slot
/// pressed; `{ "interval": 1.5 }`, seconds of game time between uses; or
/// `{ "manual": {} }`, [`use_ability`] and the `ability` cue action alone.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Trigger {
    /// [`use_ability`] and the `ability` cue action alone.
    Manual,
    /// The intent slot of this name, when pressed.
    Action(String),
    /// Every this many seconds of game time, at most once a step.
    Interval(f32),
}

/// What an ability aims at. A match outside this crate needs a wildcard
/// arm, since later forms are additions.
///
/// Data form, one field: `{ "self": {} }`, `{ "aim": {} }`,
/// `{ "nearest": { "all": ["team.enemy"], "range": 320 } }` or
/// `{ "random": { "all": ["team.enemy"], "range": 320 } }`.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum AbilityTarget {
    /// The entity itself, at its point, in the direction its [`Aim`] or its
    /// [`Intent`] gives, when either does.
    Itself,
    /// The entity itself, in the direction its [`Aim`] or its [`Intent`]
    /// gives; no direction, no use.
    Aim,
    /// The nearest live entity whose tags pass the filter, within the
    /// range.
    Nearest {
        /// The tags it carries; names an `all` or an `any` tag.
        filter: TagFilter,
        /// World pixels from the entity; any distance when `None`.
        range: Option<f32>,
    },
    /// One of the live entities whose tags pass the filter, within the
    /// range, drawn from the kit's random stream.
    Random {
        /// The tags it carries; names an `all` or an `any` tag.
        filter: TagFilter,
        /// World pixels from the entity; any distance when `None`.
        range: Option<f32>,
    },
}

/// One thing a use does. A match outside this crate needs a wildcard arm,
/// since later payloads are additions.
///
/// Data form, one field: `{ "spawn": { "prefab": "bolt", "speed": 420, "offset": 12 } }`,
/// `{ "effect": "swift" }`, `{ "impulse": 600 }` or `{ "hitbox": 0.2 }`.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Payload {
    /// A prefab spawned from the entity at its point plus `offset` along
    /// the direction, with a [`LinearMotion`] of `speed` along it when
    /// `speed` is above 0 (the prefab then holds a `linear_motion`, which
    /// the load checks).
    Spawn {
        /// The manifest prefab ID.
        prefab: String,
        /// World pixels per second along the direction; 0 leaves the
        /// prefab's own motion.
        speed: f32,
        /// World pixels from the entity's point along the direction.
        offset: f32,
    },
    /// An effect queued on the target entity, from the entity.
    Effect(DataRef<Effect>),
    /// The entity pushed along the direction at this speed, through its
    /// [`Knockback`](crate::Knockback).
    Impulse(f32),
    /// The entity's own [`Hitbox`] armed for this many seconds.
    Hitbox(f32),
}

impl Payload {
    /// Whether it needs a direction.
    fn needs_direction(&self) -> bool {
        match self {
            Payload::Spawn { speed, .. } => *speed > 0.0,
            Payload::Impulse(_) => true,
            Payload::Effect(_) | Payload::Hitbox(_) => false,
        }
    }
}

/// An ability's charges: a use takes one, time gives them back.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Charges {
    max: u32,
    refill: f32,
    all: bool,
}

/// An ability's definition: what fires it, what it costs, how long it waits
/// between uses, its charges, what it aims at and what a use does. The
/// `abilities` kind's entry or an ability in place in an entity's
/// `abilities` list.
///
/// Data form:
///
/// ```json
/// {
///   "trigger": { "action": "attack" },
///   "cost": { "stat": "mana", "amount": 10 },
///   "cooldown": 0.8,
///   "charges": { "max": 6, "refill": 1.5, "all": false },
///   "target": { "nearest": { "all": ["team.enemy"], "range": 320 } },
///   "payload": [{ "spawn": { "prefab": "bolt", "speed": 420, "offset": 12 } }, { "effect": "swift" }]
/// }
/// ```
///
/// `trigger` `manual`, `cost` none, `cooldown` 0, `charges` none and
/// `target` `self` by default; `payload` at least one, run in order. The
/// load checks each prefab (with a `linear_motion` when it spawns with a
/// speed), each effect and each target's tags.
///
/// # Examples
///
/// ```
/// use tungsten_core::DataRef;
/// use tungsten_kit::{Ability, AbilityTarget, Payload, TagFilter, Trigger};
///
/// let wand = Ability::new()
///     .with_trigger(Trigger::Interval(0.5))
///     .with_cost("mana", 2.0)
///     .with_cooldown(0.2)
///     .with_charges(3, 1.5, false)
///     .with_target(AbilityTarget::Nearest {
///         filter: TagFilter::new().all(["team.enemy"]),
///         range: Some(320.0),
///     })
///     .with_payload(Payload::Spawn { prefab: "bolt".to_string(), speed: 420.0, offset: 12.0 })
///     .with_payload(Payload::Effect(DataRef::by_id("swift")));
/// assert_eq!(wand.cooldown(), 0.2);
/// assert_eq!(wand.charges(), Some((3, 1.5, false)));
/// assert_eq!(wand.payload().len(), 2);
/// let data = serde_json::to_value(&wand).unwrap();
/// assert_eq!(data["trigger"], serde_json::json!({ "interval": 0.5 }));
/// assert_eq!(serde_json::from_value::<Ability>(data).unwrap(), wand);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "AbilityDef", into = "AbilityDef")]
pub struct Ability {
    trigger: Trigger,
    cost: Option<(String, f32)>,
    cooldown: f32,
    charges: Option<Charges>,
    target: AbilityTarget,
    payload: Vec<Payload>,
}

impl Default for Ability {
    /// [`new`](Self::new).
    fn default() -> Self {
        Self::new()
    }
}

impl Ability {
    /// A manual ability with no cost, no cooldown, no charges, aimed at the
    /// entity itself, doing nothing.
    #[must_use]
    pub fn new() -> Self {
        Self {
            trigger: Trigger::Manual,
            cost: None,
            cooldown: 0.0,
            charges: None,
            target: AbilityTarget::Itself,
            payload: Vec::new(),
        }
    }

    /// The ability, fired by `trigger`.
    ///
    /// # Panics
    ///
    /// When an interval is not finite and above 0, or an action is not an
    /// event name (`[a-z][a-z0-9_]*`).
    #[must_use]
    #[track_caller]
    pub fn with_trigger(mut self, trigger: Trigger) -> Self {
        match &trigger {
            Trigger::Interval(seconds) => assert!(
                seconds.is_finite() && *seconds > 0.0,
                "Ability::with_trigger: the interval must be finite and > 0 (got {seconds})"
            ),
            Trigger::Action(slot) => assert!(
                valid_stat(slot),
                "Ability::with_trigger: `{slot}` is not a slot name (`[a-z][a-z0-9_]*`)"
            ),
            Trigger::Manual => {}
        }
        self.trigger = trigger;
        self
    }

    /// The ability, spending `amount` of the stat `stat`'s base per use; a
    /// base below it refuses the use.
    ///
    /// # Panics
    ///
    /// When `stat` is not `[a-z][a-z0-9_]*` or `amount` is not finite and
    /// at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_cost(mut self, stat: &str, amount: f32) -> Self {
        assert!(
            valid_stat(stat),
            "Ability::with_cost: `{stat}` is not a stat name (`[a-z][a-z0-9_]*`)"
        );
        assert!(
            amount.is_finite() && amount >= 0.0,
            "Ability::with_cost: amount must be finite and >= 0 (got {amount})"
        );
        self.cost = Some((stat.to_owned(), amount));
        self
    }

    /// The ability, waiting `seconds` between uses, counted at the
    /// entity's `cooldown_rate` stat's rate.
    ///
    /// # Panics
    ///
    /// When `seconds` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_cooldown(mut self, seconds: f32) -> Self {
        assert!(
            seconds.is_finite() && seconds >= 0.0,
            "Ability::with_cooldown: seconds must be finite and >= 0 (got {seconds})"
        );
        self.cooldown = seconds;
        self
    }

    /// The ability, with `max` charges, a use taking one: one comes back
    /// every `refill` seconds of game time, or, with `all`, every one
    /// `refill` seconds after the last runs out.
    ///
    /// # Panics
    ///
    /// When `max` is 0 or `refill` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_charges(mut self, max: u32, refill: f32, all: bool) -> Self {
        assert!(max >= 1, "Ability::with_charges: max must be at least 1");
        assert!(
            refill.is_finite() && refill > 0.0,
            "Ability::with_charges: refill must be finite and > 0 (got {refill})"
        );
        self.charges = Some(Charges { max, refill, all });
        self
    }

    /// The ability, aimed at `target`.
    ///
    /// # Panics
    ///
    /// When a `nearest` or `random` filter names no `all` and no `any` tag,
    /// or its range is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_target(mut self, target: AbilityTarget) -> Self {
        if let AbilityTarget::Nearest { filter, range } | AbilityTarget::Random { filter, range } =
            &target
        {
            assert!(
                !filter.matches(None),
                "Ability::with_target: the filter names no `all` or `any` tag, so untagged entities would pass it"
            );
            if let Some(range) = range {
                assert!(
                    range.is_finite() && *range > 0.0,
                    "Ability::with_target: range must be finite and > 0 (got {range})"
                );
            }
        }
        self.target = target;
        self
    }

    /// The ability, doing `payload` after its earlier payloads.
    ///
    /// # Panics
    ///
    /// When a spawn's speed or offset is not finite, its speed is below 0,
    /// an impulse is not finite and at least 0, or a hitbox window is not
    /// finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_payload(mut self, payload: Payload) -> Self {
        match &payload {
            Payload::Spawn { speed, offset, .. } => assert!(
                speed.is_finite() && *speed >= 0.0 && offset.is_finite(),
                "Ability::with_payload: a spawn's speed must be finite and >= 0 and its offset finite (got {speed}, {offset})"
            ),
            Payload::Impulse(speed) => assert!(
                speed.is_finite() && *speed >= 0.0,
                "Ability::with_payload: an impulse must be finite and >= 0 (got {speed})"
            ),
            Payload::Hitbox(seconds) => assert!(
                seconds.is_finite() && *seconds > 0.0,
                "Ability::with_payload: a hitbox window must be finite and > 0 (got {seconds})"
            ),
            Payload::Effect(_) => {}
        }
        self.payload.push(payload);
        self
    }

    /// What fires it.
    #[must_use]
    pub fn trigger(&self) -> &Trigger {
        &self.trigger
    }

    /// The stat and amount a use spends, when it costs.
    #[must_use]
    pub fn cost(&self) -> Option<(&str, f32)> {
        self.cost
            .as_ref()
            .map(|(stat, amount)| (stat.as_str(), *amount))
    }

    /// Seconds between uses.
    #[must_use]
    pub fn cooldown(&self) -> f32 {
        self.cooldown
    }

    /// Its charges, refill and whether they refill together, when it has
    /// charges.
    #[must_use]
    pub fn charges(&self) -> Option<(u32, f32, bool)> {
        self.charges.map(|c| (c.max, c.refill, c.all))
    }

    /// What it aims at.
    #[must_use]
    pub fn target(&self) -> &AbilityTarget {
        &self.target
    }

    /// What a use does, in order.
    #[must_use]
    pub fn payload(&self) -> &[Payload] {
        &self.payload
    }

    /// Whether a use needs a direction.
    fn needs_direction(&self) -> bool {
        matches!(self.target, AbilityTarget::Aim)
            || self.payload.iter().any(Payload::needs_direction)
    }

    /// The load's check: each payload's prefab (with a `linear_motion`
    /// when it spawns with a speed) and effect, and each target's tags.
    pub(crate) fn check(&self, world: &World) -> Result<(), MissingAsset> {
        if let AbilityTarget::Nearest { filter, .. } | AbilityTarget::Random { filter, .. } =
            &self.target
        {
            let form = match self.target {
                AbilityTarget::Random { .. } => "random",
                _ => "nearest",
            };
            for (field, name) in filter_names(filter) {
                check_declared(world, &format!("target.{form}.{field}"), &name)?;
            }
        }
        for (i, payload) in self.payload.iter().enumerate() {
            match payload {
                Payload::Spawn { prefab, speed, .. } => {
                    let field = format!("payload.{i}.spawn.prefab");
                    world.check_prefab_id(&field, prefab)?;
                    let moves = world
                        .get_resource::<PrefabRegistry>()
                        .and_then(|prefabs| prefabs.get(prefab))
                        .is_some_and(|data| data.components.contains_key(LINEAR_MOTION));
                    if *speed > 0.0 && !moves {
                        return Err(MissingAsset::new(
                            field,
                            "prefab with a `linear_motion`, for a spawn with a speed",
                            prefab.as_str(),
                        ));
                    }
                }
                Payload::Effect(effect) => effect.check(world, &format!("payload.{i}.effect"))?,
                Payload::Impulse(_) | Payload::Hitbox(_) => {}
            }
        }
        Ok(())
    }
}

/// A filter's lists as its data form writes them: `all`, `any` and
/// `none`, each sorted.
fn filter_lists(filter: &TagFilter) -> [Vec<String>; 3] {
    let data = serde_json::to_value(filter).unwrap_or_default();
    ["all", "any", "none"].map(|list| {
        data.get(list)
            .and_then(|values| values.as_array())
            .map(|values| {
                values
                    .iter()
                    .filter_map(|value| value.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    })
}

/// A filter's names with the field that holds each (`all.0`), for a load's
/// check.
fn filter_names(filter: &TagFilter) -> Vec<(String, String)> {
    let [all, any, none] = filter_lists(filter);
    [("all", all), ("any", any), ("none", none)]
        .into_iter()
        .flat_map(|(list, names)| {
            names
                .into_iter()
                .enumerate()
                .map(move |(i, name)| (format!("{list}.{i}"), name))
        })
        .collect()
}

/// An ability as data writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AbilityDef {
    #[serde(default)]
    trigger: TriggerDef,
    #[serde(default)]
    cost: Option<CostDef>,
    #[serde(default, deserialize_with = "non_negative")]
    cooldown: f32,
    #[serde(default)]
    charges: Option<ChargesDef>,
    #[serde(default)]
    target: TargetDef,
    payload: Vec<PayloadDef>,
}

/// `{}`: a form that takes no value.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}

/// A trigger as data writes it: exactly one field.
#[derive(Serialize, Deserialize)]
#[serde(
    rename_all = "snake_case",
    deny_unknown_fields,
    try_from = "TriggerForm"
)]
enum TriggerDef {
    Manual(Empty),
    Action(String),
    Interval(f32),
}

/// A trigger's form, before its value is checked.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum TriggerForm {
    Manual(Empty),
    Action(String),
    Interval(#[serde(deserialize_with = "positive")] f32),
}

impl TryFrom<TriggerForm> for TriggerDef {
    type Error = String;

    fn try_from(form: TriggerForm) -> Result<Self, Self::Error> {
        Ok(match form {
            TriggerForm::Manual(empty) => TriggerDef::Manual(empty),
            TriggerForm::Action(slot) => {
                if !valid_stat(&slot) {
                    return Err(format!(
                        "action: `{slot}` is not a slot name (`[a-z][a-z0-9_]*`)"
                    ));
                }
                TriggerDef::Action(slot)
            }
            TriggerForm::Interval(seconds) => TriggerDef::Interval(seconds),
        })
    }
}

impl Default for TriggerDef {
    fn default() -> Self {
        TriggerDef::Manual(Empty {})
    }
}

/// A cost as data writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "CostFields")]
struct CostDef {
    stat: String,
    amount: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CostFields {
    stat: String,
    #[serde(deserialize_with = "non_negative")]
    amount: f32,
}

impl TryFrom<CostFields> for CostDef {
    type Error = String;

    fn try_from(fields: CostFields) -> Result<Self, Self::Error> {
        if !valid_stat(&fields.stat) {
            return Err(format!(
                "stat: `{}` is not a stat name (`[a-z][a-z0-9_]*`)",
                fields.stat
            ));
        }
        Ok(Self {
            stat: fields.stat,
            amount: fields.amount,
        })
    }
}

/// Charges as data writes them.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargesDef {
    #[serde(deserialize_with = "at_least_one")]
    max: u32,
    #[serde(deserialize_with = "positive")]
    refill: f32,
    #[serde(default)]
    all: bool,
}

/// A whole number of at least 1, for `deserialize_with`.
fn at_least_one<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<u32, D::Error> {
    use serde::de::Error as _;
    let value = u32::deserialize(deserializer)?;
    if value >= 1 {
        Ok(value)
    } else {
        Err(D::Error::custom(format!(
            "must be at least 1 (got {value})"
        )))
    }
}

/// A `nearest` or `random` target's filter and range as data writes them,
/// as the motion targets' `nearest` shape.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "NearestFields")]
struct NearestDef {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    all: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    any: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    none: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    range: Option<f32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NearestFields {
    #[serde(default)]
    all: Vec<String>,
    #[serde(default)]
    any: Vec<String>,
    #[serde(default)]
    none: Vec<String>,
    #[serde(default)]
    range: Option<f32>,
}

impl TryFrom<NearestFields> for NearestDef {
    type Error = String;

    fn try_from(fields: NearestFields) -> Result<Self, Self::Error> {
        for (list, names) in [
            ("all", &fields.all),
            ("any", &fields.any),
            ("none", &fields.none),
        ] {
            if let Some(name) = names.iter().find(|name| !valid_tag(name)) {
                return Err(format!(
                    "{list}: `{name}` is not a tag name (`[a-z][a-z0-9_]*` segments joined by dots)"
                ));
            }
        }
        if fields.all.is_empty() && fields.any.is_empty() {
            return Err(
                "names no `all` or `any` tag, so untagged entities would pass it".to_string(),
            );
        }
        if let Some(range) = fields.range
            && !(range.is_finite() && range > 0.0)
        {
            return Err(format!("range: must be finite and > 0 (got {range})"));
        }
        Ok(Self {
            all: fields.all,
            any: fields.any,
            none: fields.none,
            range: fields.range,
        })
    }
}

impl NearestDef {
    fn into_filter(self) -> (TagFilter, Option<f32>) {
        (
            TagFilter::new().all(self.all).any(self.any).none(self.none),
            self.range,
        )
    }

    fn from_filter(filter: &TagFilter, range: Option<f32>) -> Self {
        let [all, any, none] = filter_lists(filter);
        Self {
            all,
            any,
            none,
            range,
        }
    }
}

/// A target as data writes it: exactly one field.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum TargetDef {
    #[serde(rename = "self")]
    Itself(Empty),
    Aim(Empty),
    Nearest(NearestDef),
    Random(NearestDef),
}

impl Default for TargetDef {
    fn default() -> Self {
        TargetDef::Itself(Empty {})
    }
}

/// A payload as data writes it: exactly one field.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum PayloadDef {
    Spawn(SpawnDef),
    Effect(DataRef<Effect>),
    Impulse(#[serde(deserialize_with = "non_negative")] f32),
    Hitbox(#[serde(deserialize_with = "positive")] f32),
}

/// A spawn payload as data writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SpawnDef {
    prefab: String,
    #[serde(default, deserialize_with = "non_negative")]
    speed: f32,
    #[serde(default, deserialize_with = "crate::foundation::finite")]
    offset: f32,
}

impl TryFrom<AbilityDef> for Ability {
    type Error = String;

    fn try_from(def: AbilityDef) -> Result<Self, Self::Error> {
        if def.payload.is_empty() {
            return Err("payload: at least one".to_string());
        }
        Ok(Self {
            trigger: match def.trigger {
                TriggerDef::Manual(_) => Trigger::Manual,
                TriggerDef::Action(slot) => Trigger::Action(slot),
                TriggerDef::Interval(seconds) => Trigger::Interval(seconds),
            },
            cost: def.cost.map(|cost| (cost.stat, cost.amount)),
            cooldown: def.cooldown,
            charges: def.charges.map(|c| Charges {
                max: c.max,
                refill: c.refill,
                all: c.all,
            }),
            target: match def.target {
                TargetDef::Itself(_) => AbilityTarget::Itself,
                TargetDef::Aim(_) => AbilityTarget::Aim,
                TargetDef::Nearest(nearest) => {
                    let (filter, range) = nearest.into_filter();
                    AbilityTarget::Nearest { filter, range }
                }
                TargetDef::Random(nearest) => {
                    let (filter, range) = nearest.into_filter();
                    AbilityTarget::Random { filter, range }
                }
            },
            payload: def
                .payload
                .into_iter()
                .map(|payload| match payload {
                    PayloadDef::Spawn(spawn) => Payload::Spawn {
                        prefab: spawn.prefab,
                        speed: spawn.speed,
                        offset: spawn.offset,
                    },
                    PayloadDef::Effect(effect) => Payload::Effect(effect),
                    PayloadDef::Impulse(speed) => Payload::Impulse(speed),
                    PayloadDef::Hitbox(seconds) => Payload::Hitbox(seconds),
                })
                .collect(),
        })
    }
}

impl From<Ability> for AbilityDef {
    fn from(ability: Ability) -> Self {
        Self {
            trigger: match ability.trigger {
                Trigger::Manual => TriggerDef::Manual(Empty {}),
                Trigger::Action(slot) => TriggerDef::Action(slot),
                Trigger::Interval(seconds) => TriggerDef::Interval(seconds),
            },
            cost: ability.cost.map(|(stat, amount)| CostDef { stat, amount }),
            cooldown: ability.cooldown,
            charges: ability.charges.map(|c| ChargesDef {
                max: c.max,
                refill: c.refill,
                all: c.all,
            }),
            target: match &ability.target {
                AbilityTarget::Itself => TargetDef::Itself(Empty {}),
                AbilityTarget::Aim => TargetDef::Aim(Empty {}),
                AbilityTarget::Nearest { filter, range } => {
                    TargetDef::Nearest(NearestDef::from_filter(filter, *range))
                }
                AbilityTarget::Random { filter, range } => {
                    TargetDef::Random(NearestDef::from_filter(filter, *range))
                }
            },
            payload: ability
                .payload
                .into_iter()
                .map(|payload| match payload {
                    Payload::Spawn {
                        prefab,
                        speed,
                        offset,
                    } => PayloadDef::Spawn(SpawnDef {
                        prefab,
                        speed,
                        offset,
                    }),
                    Payload::Effect(effect) => PayloadDef::Effect(effect),
                    Payload::Impulse(speed) => PayloadDef::Impulse(speed),
                    Payload::Hitbox(seconds) => PayloadDef::Hitbox(seconds),
                })
                .collect(),
        }
    }
}

/// One ability an entity owns, with its state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "EntryFields", into = "EntryFields")]
struct Entry {
    name: String,
    ability: DataRef<Ability>,
    cooldown_left: f32,
    /// `None` for an ability without charges.
    charges: Option<u32>,
    /// Game seconds to the next charge, while one is missing.
    refill_left: f32,
    /// Game seconds to the next use of an interval trigger.
    interval_left: f32,
}

/// An entry as a save writes it, before its values are checked.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EntryFields {
    name: String,
    ability: DataRef<Ability>,
    #[serde(default)]
    cooldown_left: f32,
    #[serde(default)]
    charges: Option<u32>,
    #[serde(default)]
    refill_left: f32,
    #[serde(default)]
    interval_left: f32,
}

impl TryFrom<EntryFields> for Entry {
    type Error = String;

    fn try_from(fields: EntryFields) -> Result<Self, Self::Error> {
        if !valid_name(&fields.name) {
            return Err(format!(
                "name: `{}` is not an entry ID or `#<index>`",
                fields.name
            ));
        }
        for (field, value) in [
            ("cooldown_left", fields.cooldown_left),
            ("refill_left", fields.refill_left),
            ("interval_left", fields.interval_left),
        ] {
            if !(value.is_finite() && value >= 0.0) {
                return Err(format!("{field}: must be finite and >= 0 (got {value})"));
            }
        }
        Ok(Self {
            name: fields.name,
            ability: fields.ability,
            cooldown_left: fields.cooldown_left,
            charges: fields.charges,
            refill_left: fields.refill_left,
            interval_left: fields.interval_left,
        })
    }
}

impl From<Entry> for EntryFields {
    fn from(entry: Entry) -> Self {
        Self {
            name: entry.name,
            ability: entry.ability,
            cooldown_left: entry.cooldown_left,
            charges: entry.charges,
            refill_left: entry.refill_left,
            interval_left: entry.interval_left,
        }
    }
}

/// Whether `name` is an ability's name: an entry ID, or `#<index>`.
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && (name
            .strip_prefix('#')
            .is_some_and(|index| !index.is_empty() && index.bytes().all(|b| b.is_ascii_digit()))
            || !name.starts_with('#'))
}

/// The abilities an entity owns, by name: an entry's ID, or `#<index>` in
/// the list for an ability written in place. Names are unique in one
/// component: [`with`](Self::with) of an owned name replaces that entry,
/// and a scene list naming an ID twice fails the load. Each holds its
/// cooldown, its charges and its interval, which [`ABILITY_TICK`] counts;
/// it is data, saved with that state.
///
/// Scene name `abilities`: `["wand", { "trigger": { "action": "dash" }, "payload": [{ "impulse": 600 }] }]`.
///
/// # Examples
///
/// ```
/// use tungsten_core::DataRef;
/// use tungsten_kit::{Abilities, Ability, Payload};
///
/// let dash = Ability::new().with_cooldown(0.8).with_charges(2, 1.0, false).with_payload(Payload::Impulse(600.0));
/// let abilities = Abilities::new().with(DataRef::by_id("wand")).with(DataRef::inline(dash));
/// assert_eq!(abilities.names().collect::<Vec<_>>(), ["wand", "#1"]);
/// assert!(abilities.ready("wand") && abilities.ready("#1"));
/// assert_eq!(abilities.charges("#1"), None, "full, counted at the first tick");
/// assert_eq!(abilities.cooldown_left("#1"), Some(0.0));
/// assert_eq!(abilities.cooldown_left("sword"), None, "not owned");
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "AbilitiesSave", into = "AbilitiesSave")]
pub struct Abilities {
    /// In list order.
    entries: Vec<Entry>,
}

/// The component as a save writes it.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct AbilitiesSave {
    entries: Vec<Entry>,
}

impl TryFrom<AbilitiesSave> for Abilities {
    type Error = String;

    fn try_from(save: AbilitiesSave) -> Result<Self, Self::Error> {
        for (i, entry) in save.entries.iter().enumerate() {
            if save.entries[..i]
                .iter()
                .any(|other| other.name == entry.name)
            {
                return Err(format!("entries.{i}: `{}` is named twice", entry.name));
            }
        }
        Ok(Self {
            entries: save.entries,
        })
    }
}

impl From<Abilities> for AbilitiesSave {
    fn from(abilities: Abilities) -> Self {
        Self {
            entries: abilities.entries,
        }
    }
}

impl Abilities {
    /// No abilities.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The abilities, with `ability` [`add`](Self::add)ed.
    #[must_use]
    pub fn with(mut self, ability: DataRef<Ability>) -> Self {
        self.add(ability);
        self
    }

    /// Adds `ability` under its entry's ID, or `#<index>` for one written
    /// in place, ready to use with every charge; an owned name's entry is
    /// replaced.
    pub fn add(&mut self, ability: DataRef<Ability>) {
        let name = ability
            .id()
            .map_or_else(|| format!("#{}", self.entries.len()), str::to_owned);
        let entry = Entry {
            name,
            ability,
            cooldown_left: 0.0,
            charges: None,
            refill_left: 0.0,
            interval_left: 0.0,
        };
        match self.entries.iter().position(|e| e.name == entry.name) {
            Some(at) => self.entries[at] = entry,
            None => self.entries.push(entry),
        }
    }

    /// The names, in list order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|entry| entry.name.as_str())
    }

    /// Whether `name` is owned, its cooldown has passed and a charge is
    /// left.
    #[must_use]
    pub fn ready(&self, name: &str) -> bool {
        self.find(name).is_some_and(|entry| {
            entry.cooldown_left <= 0.0 && entry.charges.is_none_or(|charges| charges > 0)
        })
    }

    /// Seconds of cooldown left on `name`; `None` when not owned.
    #[must_use]
    pub fn cooldown_left(&self, name: &str) -> Option<f32> {
        self.find(name).map(|entry| entry.cooldown_left)
    }

    /// The charges left on `name`; `None` when not owned, without
    /// charges, or full and not yet counted: an entry reads its definition's
    /// charges at its first tick or use, and after a
    /// [`refill`](Self::refill).
    #[must_use]
    pub fn charges(&self, name: &str) -> Option<u32> {
        self.find(name).and_then(|entry| entry.charges)
    }

    /// Makes `name` ready now: every charge back and its cooldown cleared;
    /// whether it is owned.
    pub fn refill(&mut self, name: &str) -> bool {
        let Some(entry) = self.find_mut(name) else {
            return false;
        };
        entry.cooldown_left = 0.0;
        entry.refill_left = 0.0;
        entry.charges = None;
        true
    }

    fn find(&self, name: &str) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.name == name)
    }

    fn find_mut(&mut self, name: &str) -> Option<&mut Entry> {
        self.entries.iter_mut().find(|entry| entry.name == name)
    }
}

/// `components.abilities` as a scene writes it: the entries, names unique.
#[derive(Serialize, Deserialize)]
#[serde(try_from = "Vec<DataRef<Ability>>")]
struct AbilitiesDef(Vec<DataRef<Ability>>);

impl TryFrom<Vec<DataRef<Ability>>> for AbilitiesDef {
    type Error = String;

    fn try_from(list: Vec<DataRef<Ability>>) -> Result<Self, Self::Error> {
        for (i, ability) in list.iter().enumerate() {
            if let Some(id) = ability.id()
                && list[..i].iter().any(|other| other.id() == Some(id))
            {
                return Err(format!("{i}: `{id}` is named twice"));
            }
        }
        Ok(Self(list))
    }
}

impl AbilitiesDef {
    fn check(&self, world: &World) -> Result<(), MissingAsset> {
        for (i, ability) in self.0.iter().enumerate() {
            ability.check(world, &i.to_string())?;
        }
        Ok(())
    }
}

/// What [`use_ability`] did. A match outside this crate needs a wildcard
/// arm, since later outcomes are additions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Used {
    /// The cost, a charge and the cooldown were spent and the payloads ran.
    Used,
    /// The cooldown has not passed.
    NotReady,
    /// No charge is left.
    NoCharge,
    /// The cost's stat is missing or its base is below the amount.
    NoCost,
    /// Nothing to aim at, or no direction for a payload that needs one.
    NoTarget,
    /// The entity does not own an ability of that name, or its entry is
    /// missing.
    NoAbility,
}

/// Uses the ability `name` of `entity` now, as a trigger or the `ability`
/// cue action uses it: its target is resolved first (`nearest` and
/// `random` over the live, undespawned entities whose tags pass the filter
/// within range, not dead by a [`Health`]; `aim` and a `spawn` with a speed
/// or an `impulse` need a direction, from the entity's [`Aim`], else its
/// [`Intent`]'s aim), and nothing is spent without one. Then the cost
/// comes off the stat's base, a charge and the cooldown are spent, the
/// payloads run in order, [`AbilityUsed`] goes out and the trace records
/// `ability_used`. A payload that cannot run (no `CommandBuffer` for a
/// spawn, no [`Knockback`](crate::Knockback) for an impulse, no [`Hitbox`]
/// for a window, no effects queue) is traced and skipped.
///
/// # Examples
///
/// ```
/// use tungsten_core::{DataRef, EventQueue, Plugin, Schedule, World};
/// use tungsten_kit::{Abilities, AbilitiesPlugin, Ability, AbilityUsed, Payload, Stats, Used, use_ability};
///
/// let mut world = World::new();
/// let mut schedule = Schedule::new();
/// AbilitiesPlugin.build(&mut schedule, &mut world);
/// let shout = Ability::new().with_cost("mana", 3.0).with_cooldown(1.0).with_payload(Payload::Hitbox(0.2));
/// let bard = world.spawn_with((
///     Abilities::new().with(DataRef::inline(shout)),
///     Stats::new().with("mana", 5.0),
/// ));
/// assert_eq!(use_ability(&mut world, bard, "#0"), Used::Used, "no hitbox to arm, still a use");
/// assert_eq!(world.get::<Stats>(bard).unwrap().base("mana"), Some(2.0));
/// assert_eq!(use_ability(&mut world, bard, "#0"), Used::NotReady);
/// assert_eq!(use_ability(&mut world, bard, "lute"), Used::NoAbility);
/// assert_eq!(world.resource::<EventQueue<AbilityUsed>>().iter().count(), 1);
/// ```
pub fn use_ability(world: &mut World, entity: Entity, name: &str) -> Used {
    let mut scratch = Scratch::default();
    use_with(world, entity, name, &mut scratch)
}

/// An ability used, sent once [`AbilitiesPlugin`] has registered its
/// queue: the cue event `ability_used`, whose other entity is the target.
/// Build one with [`AbilityUsed::new`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct AbilityUsed {
    /// The entity that used it.
    pub entity: Entity,
    /// The ability's name in the entity's [`Abilities`].
    pub name: String,
    /// The entity it aimed at: the target of `nearest` and `random`, the
    /// entity itself otherwise.
    pub target: Option<Entity>,
    /// The entity's point at the use.
    pub position: Vec2,
}

impl AbilityUsed {
    /// An event with these fields and no target.
    #[must_use]
    pub fn new(entity: Entity, name: impl Into<String>, position: Vec2) -> Self {
        Self {
            entity,
            name: name.into(),
            target: None,
            position,
        }
    }

    /// The event, aimed at `target`.
    #[must_use]
    pub fn with_target(mut self, target: Entity) -> Self {
        self.target = Some(target);
        self
    }
}

/// Why a use or a payload did not run, for the trace.
const NO_COMMANDS: &str = "the World has no CommandBuffer";
const NO_KNOCKBACK: &str = "the entity has no Knockback, or is dead";
const NO_HITBOX: &str = "the entity has no Hitbox";
const NO_EFFECTS: &str = "the World has no EffectsPlugin queue";
const NO_PREFAB: &str = "the prefab does not prepare";

/// Traces why `action` of the ability did not run on `entity`.
fn skip(world: &mut World, entity: Entity, action: &'static str, why: &'static str) {
    trace(world, entity, || TraceKind::Skipped {
        event: "ability",
        action,
        why,
    });
}

/// `ability`'s definition: the one in place, or the entry the World's data
/// holds now.
fn definition<'a>(world: &'a World, ability: &'a DataRef<Ability>) -> Option<&'a Ability> {
    match world.get_resource::<DataRegistry>() {
        Some(data) => ability.resolve(data),
        None => ability.as_inline(),
    }
}

/// The scratch a use or a tick keeps between calls: the matches of each
/// distinct filter this run, and the entities to use this step.
#[derive(Default)]
struct Scratch {
    /// Each filter's eligible matches, gathered once per run.
    filters: Vec<(TagFilter, Vec<(Entity, Vec2)>)>,
    used: usize,
    /// The entities with abilities this step, in index order.
    entities: Vec<Entity>,
    /// The names to use this step for one entity.
    uses: Vec<String>,
}

impl Scratch {
    /// Starts a run: the last run's matches are dropped, their storage
    /// kept.
    fn begin(&mut self) {
        self.used = 0;
    }

    /// The live entities whose tags pass `filter`, with their points,
    /// sorted by index, `seeker` left out: alive, not despawning, not dead
    /// by a [`Health`], gathered at the run's first call for the filter.
    fn matches(&mut self, world: &World, filter: &TagFilter) -> &[(Entity, Vec2)] {
        let index = if let Some(index) = self.filters[..self.used]
            .iter()
            .position(|(known, _)| known == filter)
        {
            index
        } else {
            if self.used == self.filters.len() {
                self.filters.push((filter.clone(), Vec::new()));
            } else {
                self.filters[self.used].0.clone_from(filter);
            }
            let found = &mut self.filters[self.used].1;
            found.clear();
            found.extend(
                world
                    .query::<(
                        Entity,
                        &Tags,
                        Option<&Position>,
                        Option<&Transform>,
                        Option<&Health>,
                    )>()
                    .filter(|(entity, tags, _, _, health)| {
                        filter.matches(Some(tags))
                            && !world.has::<Despawning>(*entity)
                            && !health.is_some_and(Health::is_dead)
                    })
                    .filter_map(|(entity, _, position, transform, _)| {
                        let at = position.map(|p| p.0).or(transform.map(|t| t.position))?;
                        Some((entity, at))
                    }),
            );
            found.sort_by_key(|(entity, _)| entity.id());
            self.used += 1;
            self.used - 1
        };
        &self.filters[index].1
    }
}

/// The direction `entity` aims in: its [`Aim`]'s, else its [`Intent`]'s
/// aim, normalized.
fn aimed(world: &World, entity: Entity) -> Option<Vec2> {
    world
        .get::<Aim>(entity)
        .and_then(Aim::direction)
        .or_else(|| {
            world
                .get::<Intent>(entity)
                .and_then(Intent::aim)
                .and_then(Vec2::try_normalize)
        })
}

/// What a use aims at: the target entity (the entity itself for `self` and
/// `aim`) and the direction, when there is one.
fn resolve_target(
    world: &mut World,
    entity: Entity,
    at: Vec2,
    target: &AbilityTarget,
    scratch: &mut Scratch,
) -> Option<(Entity, Option<Vec2>)> {
    match target {
        AbilityTarget::Itself => Some((entity, aimed(world, entity))),
        AbilityTarget::Aim => aimed(world, entity).map(|direction| (entity, Some(direction))),
        AbilityTarget::Nearest { filter, range } => {
            let limit = range.map_or(f32::INFINITY, |range| range * range);
            let mut nearest: Option<(Entity, Vec2, f32)> = None;
            for &(other, point) in scratch.matches(world, filter) {
                let distance = point.distance_squared(at);
                if other != entity
                    && distance <= limit
                    && nearest.is_none_or(|(_, _, best)| distance < best)
                {
                    nearest = Some((other, point, distance));
                }
            }
            let (other, point, _) = nearest?;
            Some((other, (point - at).try_normalize()))
        }
        AbilityTarget::Random { filter, range } => {
            let limit = range.map_or(f32::INFINITY, |range| range * range);
            let mut rng = world.remove_resource::<KitRng>().unwrap_or_default();
            let picked = {
                let within: Vec<(Entity, Vec2)> = scratch
                    .matches(world, filter)
                    .iter()
                    .copied()
                    .filter(|&(other, point)| {
                        other != entity && point.distance_squared(at) <= limit
                    })
                    .collect();
                if within.is_empty() {
                    None
                } else {
                    Some(within[rng.index(within.len())])
                }
            };
            world.insert_resource(rng);
            let (other, point) = picked?;
            Some((other, (point - at).try_normalize()))
        }
    }
}

/// [`use_ability`]'s work, with `scratch`.
fn use_with(world: &mut World, entity: Entity, name: &str, scratch: &mut Scratch) -> Used {
    if !world.is_alive(entity) || world.has::<Despawning>(entity) {
        return Used::NoAbility;
    }
    let Some(entry) = world
        .get::<Abilities>(entity)
        .and_then(|abilities| abilities.find(name))
    else {
        return Used::NoAbility;
    };
    let (ability_ref, cooldown_left, charges) =
        (entry.ability.clone(), entry.cooldown_left, entry.charges);
    let Some(ability) = definition(world, &ability_ref).cloned() else {
        return Used::NoAbility;
    };
    if cooldown_left > 0.0 {
        return Used::NotReady;
    }
    let charges = match (charges, ability.charges) {
        (Some(left), _) => Some(left),
        (None, Some(c)) => Some(c.max),
        (None, None) => None,
    };
    if charges == Some(0) {
        return Used::NoCharge;
    }
    if let Some((stat, amount)) = &ability.cost {
        let base = world
            .get::<Stats>(entity)
            .and_then(|stats| stats.base(stat));
        if !base.is_some_and(|base| base >= *amount) {
            return Used::NoCost;
        }
    }
    let at = point_of(world, entity).unwrap_or(Vec2::ZERO);
    let Some((target, direction)) = resolve_target(world, entity, at, &ability.target, scratch)
    else {
        return Used::NoTarget;
    };
    if direction.is_none() && ability.needs_direction() {
        return Used::NoTarget;
    }
    // Spend.
    if let Some((stat, amount)) = &ability.cost
        && let Some(stats) = world.get_mut::<Stats>(entity)
        && let Some(base) = stats.base(stat)
    {
        stats.set_base(stat, base - amount);
    }
    {
        let abilities = world.get_mut::<Abilities>(entity).unwrap();
        let entry = abilities.find_mut(name).unwrap();
        entry.cooldown_left = ability.cooldown;
        if let (Some(left), Some(c)) = (charges, ability.charges) {
            let left = left - 1;
            entry.charges = Some(left);
            if entry.refill_left <= 0.0 && (!c.all || left == 0) {
                entry.refill_left = c.refill;
            }
        }
    }
    // The payloads, in order.
    for payload in &ability.payload {
        match payload {
            Payload::Spawn {
                prefab,
                speed,
                offset,
            } => {
                if !world.has_resource::<CommandBuffer>() {
                    skip(world, entity, "spawn", NO_COMMANDS);
                    continue;
                }
                let along = direction.unwrap_or(Vec2::ZERO);
                let position = at + along * *offset;
                match world.prepare_prefab(prefab, [position.x, position.y]) {
                    Ok(prepared) => {
                        let prepared = if *speed > 0.0 {
                            prepared.with((LinearMotion::new(along * *speed),))
                        } else {
                            prepared
                        };
                        world
                            .resource_mut::<CommandBuffer>()
                            .spawn_prefab_from(entity, prepared);
                        trace(world, entity, || TraceKind::Spawn {
                            prefab: prefab.clone(),
                            by: ABILITY,
                        });
                    }
                    Err(err) => {
                        trace(world, entity, || TraceKind::SpawnFailed {
                            prefab: prefab.clone(),
                            by: ABILITY,
                            error: err.to_string(),
                        });
                        skip(world, entity, "spawn", NO_PREFAB);
                    }
                }
            }
            Payload::Effect(effect) => {
                let request = EffectRequest::new(target, effect.clone()).with_source(entity);
                if !request.queue(world) {
                    skip(world, entity, "effect", NO_EFFECTS);
                }
            }
            Payload::Impulse(speed) => {
                let along = direction.unwrap_or(Vec2::ZERO);
                if !knock_back(world, entity, along * *speed) {
                    skip(world, entity, "impulse", NO_KNOCKBACK);
                }
            }
            Payload::Hitbox(seconds) => match world.get_mut::<Hitbox>(entity) {
                Some(hitbox) => hitbox.arm_for(*seconds),
                None => skip(world, entity, "hitbox", NO_HITBOX),
            },
        }
    }
    let other = (target != entity).then_some(target);
    if let Some(queue) = world.get_resource_mut::<EventQueue<AbilityUsed>>() {
        queue.send(AbilityUsed {
            entity,
            name: name.to_owned(),
            target: other,
            position: at,
        });
    }
    trace(world, entity, || TraceKind::Event {
        name: "ability_used",
        other,
        amount: None,
    });
    Used::Used
}

/// [`ABILITY_TICK`]'s state between steps.
#[derive(Default)]
struct Ticker {
    scratch: Scratch,
}

impl Ticker {
    /// Per entity in index order: each entry's cooldown at the entity's
    /// `cooldown_rate`, its refill and its interval on the game clock,
    /// then the uses its triggers ask for.
    fn run(&mut self, world: &mut World) {
        let dt = world
            .get_resource::<Time>()
            .map_or(0.0, Time::delta)
            .max(0.0);
        self.scratch.begin();
        self.scratch.entities.clear();
        self.scratch.entities.extend(
            world
                .query::<(Entity, &Abilities)>()
                .map(|(entity, _)| entity),
        );
        self.scratch.entities.sort_by_key(|entity| entity.id());
        let entities = std::mem::take(&mut self.scratch.entities);
        for &entity in &entities {
            self.tick_entity(world, entity, dt);
        }
        self.scratch.entities = entities;
    }

    fn tick_entity(&mut self, world: &mut World, entity: Entity, dt: f32) {
        let rate = world
            .get::<Stats>(entity)
            .and_then(|stats| stats.get(Stats::COOLDOWN_RATE))
            .map_or(1.0, |rate| rate.max(0.0));
        let Some(abilities) = world.get::<Abilities>(entity) else {
            return;
        };
        let mut abilities = abilities.clone();
        self.scratch.uses.clear();
        let pressed = |slot: &str| {
            world
                .get::<Intent>(entity)
                .is_some_and(|intent| intent.pressed(slot))
        };
        for entry in &mut abilities.entries {
            let Some(ability) = definition(world, &entry.ability) else {
                continue;
            };
            if dt > 0.0 {
                entry.cooldown_left = count_down(entry.cooldown_left, dt * rate);
                if let Some(c) = ability.charges {
                    let left = entry.charges.get_or_insert(c.max);
                    if *left < c.max {
                        if entry.refill_left <= 0.0 {
                            entry.refill_left = c.refill;
                        }
                        entry.refill_left = count_down(entry.refill_left, dt);
                        if entry.refill_left <= 0.0 {
                            *left = if c.all { c.max } else { *left + 1 };
                            if *left < c.max {
                                entry.refill_left = c.refill;
                            }
                        }
                    }
                } else {
                    entry.charges = None;
                }
            }
            match &ability.trigger {
                Trigger::Action(slot) => {
                    if pressed(slot) {
                        self.scratch.uses.push(entry.name.clone());
                    }
                }
                Trigger::Interval(seconds) => {
                    if dt > 0.0 {
                        if entry.interval_left <= 0.0 {
                            entry.interval_left = *seconds;
                        }
                        entry.interval_left = count_down(entry.interval_left, dt);
                        if entry.interval_left <= 0.0 {
                            entry.interval_left = *seconds;
                            self.scratch.uses.push(entry.name.clone());
                        }
                    }
                }
                Trigger::Manual => {}
            }
        }
        *world.get_mut::<Abilities>(entity).unwrap() = abilities;
        let uses = std::mem::take(&mut self.scratch.uses);
        for name in &uses {
            use_with(world, entity, name, &mut self.scratch);
        }
        self.scratch.uses = uses;
    }
}

/// Registers the `abilities` data kind and scene component,
/// `EventQueue<AbilityUsed>` and [`ABILITY_TICK`] in the intent phase after
/// [`ACTION_INPUT`](crate::ACTION_INPUT) and [`AIM`](crate::AIM) when they
/// are registered, so a trigger reads the step's intent and aim.
///
/// # Examples
///
/// ```
/// use tungsten_core::{Plugin, Schedule, Stage, World};
/// use tungsten_kit::{ABILITY_TICK, AbilitiesPlugin};
///
/// let mut world = World::new();
/// let mut schedule = Schedule::new();
/// AbilitiesPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
/// assert_eq!(schedule.stage_of(ABILITY_TICK), Some(Stage::FixedUpdate));
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct AbilitiesPlugin;

impl Plugin for AbilitiesPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<Abilities>(world, "Abilities");
        describe_target(world);
        describe_abilities(world);
        world.register_event::<AbilityUsed>();
        world.register_data_kind_with_assets(ABILITIES, |ability: &Ability, world: &World| {
            ability.check(world)
        });
        world.register_component_with_assets(
            ABILITIES,
            |def: &AbilitiesDef, world: &World| def.check(world),
            |def: AbilitiesDef, _: &SpawnContext<'_>| {
                let mut abilities = Abilities::new();
                for ability in def.0 {
                    abilities.add(ability);
                }
                (abilities,)
            },
        );
        let mut ticker = Ticker::default();
        schedule.add(
            Phase::Intent.stage(),
            Phase::Intent
                .join(system(ABILITY_TICK, move |world: &mut World| {
                    ticker.run(world);
                }))
                .after_if_present(ACTION_INPUT)
                .after_if_present(AIM),
        );
    }
}

/// The descriptions of `abilities`, its shapes and the `abilities` kind.
fn describe_abilities(world: &mut World) {
    describe(
        world,
        ComponentDescription::new(
            ABILITIES,
            "The abilities the entity owns, by name: an entry's ID, or `#<index>` for one written in place.",
        )
        .field(FieldDescription::new(
            "list",
            FieldType::List(Box::new(FieldType::Data(ABILITIES))),
            "the value itself: each an `abilities` entry's ID or an ability in place, names unique; the load checks each",
        ))
        .event("AbilityUsed")
        .genre(TOP_DOWN)
        .genre(SHMUP)
        .sample(r#"["wand"]"#)
        .roundtrip(round_trip::<AbilitiesDef>),
    );
    let empty = || FieldType::Object("{}");
    let shapes = [
        ComponentDescription::new(
            ABILITY,
            "What an entity can do: fired by a trigger, at a cost, with a cooldown and charges, at a target.",
        )
        .field(
            FieldDescription::new(
                "trigger",
                FieldType::Shape(ABILITY_TRIGGER),
                "what fires it",
            )
            .default(r#"{ "manual": {} }"#),
        )
        .field(
            FieldDescription::new(
                "cost",
                FieldType::Shape(ABILITY_COST),
                "a stat's base spent per use; a base below it refuses the use",
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
                "seconds between uses, counted at the entity's `cooldown_rate`",
            )
            .default("0.0"),
        )
        .field(
            FieldDescription::new(
                "charges",
                FieldType::Shape(ABILITY_CHARGES),
                "uses in hand, refilled by time",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "target",
                FieldType::Shape(ABILITY_TARGET),
                "what it aims at; the load checks a filter's tags",
            )
            .default(r#"{ "self": {} }"#),
        )
        .field(FieldDescription::new(
            "payload",
            FieldType::List(Box::new(FieldType::Shape(ABILITY_PAYLOAD))),
            "what a use does, in order; at least one",
        ))
        .sample(r#"{ "payload": [{ "effect": "swift" }] }"#)
        .roundtrip(round_trip::<AbilityDef>),
        ComponentDescription::new(ABILITY_TRIGGER, "What fires an ability.")
            .one_of()
            .field(FieldDescription::new(
                "action",
                FieldType::Text,
                "an intent slot, used when pressed",
            ))
            .field(FieldDescription::new(
                "interval",
                FieldType::F32 {
                    min: Bound::Exclusive(0.0),
                    max: Bound::Open,
                },
                "seconds of game time between uses, at most once a step",
            ))
            .field(FieldDescription::new(
                "manual",
                empty(),
                "`use_ability` and the `ability` cue action alone",
            ))
            .sample(r#"{ "manual": {} }"#)
            .roundtrip(round_trip::<TriggerDef>),
        ComponentDescription::new(ABILITY_COST, "A stat's base spent per use.")
            .field(FieldDescription::new(
                "stat",
                FieldType::Text,
                "the stat, `[a-z][a-z0-9_]*`; an entity without it cannot use the ability",
            ))
            .field(FieldDescription::new(
                "amount",
                FieldType::F32 {
                    min: Bound::Inclusive(0.0),
                    max: Bound::Open,
                },
                "taken off the base",
            ))
            .sample(r#"{ "stat": "mana", "amount": 10 }"#)
            .roundtrip(round_trip::<CostDef>),
        ComponentDescription::new(ABILITY_CHARGES, "Uses in hand, a use taking one, time giving them back.")
            .field(FieldDescription::new(
                "max",
                FieldType::U32 {
                    min: Some(1),
                    max: None,
                },
                "the charges in hand at the start and at most",
            ))
            .field(FieldDescription::new(
                "refill",
                FieldType::F32 {
                    min: Bound::Exclusive(0.0),
                    max: Bound::Open,
                },
                "seconds of game time per charge back, or until all come back with `all`",
            ))
            .field(
                FieldDescription::new(
                    "all",
                    FieldType::Bool,
                    "every charge comes back together, `refill` seconds after the last runs out",
                )
                .default("false"),
            )
            .sample(r#"{ "max": 3, "refill": 1.5 }"#)
            .roundtrip(round_trip::<ChargesDef>),
        ComponentDescription::new(ABILITY_TARGET, "What an ability aims at.")
            .one_of()
            .field(FieldDescription::new(
                "self",
                empty(),
                "the entity itself, in the direction its aim or its intent gives when either does",
            ))
            .field(FieldDescription::new(
                "aim",
                empty(),
                "the entity itself, in the direction its aim or its intent gives; no direction, no use",
            ))
            .field(FieldDescription::new(
                "nearest",
                FieldType::Shape(NEAREST_SHAPE),
                "the nearest live entity whose tags pass the filter, within the range",
            ))
            .field(FieldDescription::new(
                "random",
                FieldType::Shape(NEAREST_SHAPE),
                "one of the live entities whose tags pass the filter, within the range, from the kit's random stream",
            ))
            .sample(r#"{ "self": {} }"#)
            .roundtrip(round_trip::<TargetDef>),
        ComponentDescription::new(ABILITY_PAYLOAD, "One thing a use does.")
            .one_of()
            .field(FieldDescription::new(
                "spawn",
                FieldType::Shape(SPAWN_PAYLOAD),
                "a prefab spawned from the entity",
            ))
            .field(FieldDescription::new(
                "effect",
                FieldType::Data(EFFECTS),
                "an effect queued on the target from the entity; the load checks it",
            ))
            .field(FieldDescription::new(
                "impulse",
                FieldType::F32 {
                    min: Bound::Inclusive(0.0),
                    max: Bound::Open,
                },
                "world pixels per second the entity is pushed along the direction, through its knockback",
            ))
            .field(FieldDescription::new(
                "hitbox",
                FieldType::F32 {
                    min: Bound::Exclusive(0.0),
                    max: Bound::Open,
                },
                "seconds the entity's own hitbox is armed for",
            ))
            .sample(r#"{ "impulse": 600 }"#)
            .roundtrip(round_trip::<PayloadDef>),
        ComponentDescription::new(SPAWN_PAYLOAD, "A prefab spawned from the entity at its point.")
            .field(FieldDescription::new(
                "prefab",
                FieldType::Prefab,
                "the manifest prefab; the load checks the ID, and that it holds a `linear_motion` with a speed",
            ))
            .field(
                FieldDescription::new(
                    "speed",
                    FieldType::F32 {
                        min: Bound::Inclusive(0.0),
                        max: Bound::Open,
                    },
                    "world pixels per second along the direction, as its `linear_motion`; 0 leaves the prefab's own",
                )
                .default("0.0"),
            )
            .field(
                FieldDescription::new(
                    "offset",
                    FieldType::F32 {
                        min: Bound::Open,
                        max: Bound::Open,
                    },
                    "world pixels from the entity's point along the direction",
                )
                .default("0.0"),
            )
            .sample(r#"{ "prefab": "bolt" }"#)
            .roundtrip(round_trip::<SpawnDef>),
    ];
    for shape in shapes {
        describe_shape(world, shape);
    }
    describe_kind(
        world,
        ABILITIES,
        "An `ability`: what an entity's `abilities` list owns, a trigger or the `ability` cue action uses.",
    );
}

impl Inspectable for Abilities {
    /// Each ability in a line: its name, its cooldown and its charges.
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        self.entries
            .iter()
            .map(|entry| {
                let mut line = entry.name.clone();
                if entry.cooldown_left > 0.0 {
                    let _ = write!(line, ", {:.2} s", entry.cooldown_left);
                } else {
                    line.push_str(", ready");
                }
                if let Some(charges) = entry.charges {
                    let _ = write!(line, ", {charges} charges");
                }
                ("ability", line)
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "../tests/ability.rs"]
mod tests;
