//! Effects: timed, infinite or instant changes an entity carries, with a
//! periodic payload, stacking, modifiers on its stats and resistances,
//! granted, blocking and suppressing tags, and a transition at the end; the
//! `effects` data kind, the `effects` scene component and the requests that
//! apply them.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use tungsten_core::{
    DataRef, DataRegistry, Entity, EventQueue, Inspectable, MissingAsset, Plugin, Schedule,
    SpawnContext, Time, World, system,
};

use crate::damage::ailments::{AILMENT_FEED, register_ailments};
use crate::damage::defense::Resist;
use crate::damage::health::Health;
use crate::damage::pipeline::{DAMAGE_RESOLVE, DamagePart, DamageRequest, HealRequest};
use crate::foundation::curve::{CURVES, Curve, register_curves};
use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, GRID, PLATFORMER, describe,
    describe_kind, describe_shape, round_trip,
};
use crate::foundation::inspect::register_rows_with;
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::stats::{Modifier, STATS_APPLY, Stats};
use crate::foundation::tags::{Tags, check_declared, valid_tag};
use crate::foundation::trace::{TraceKind, trace};
use crate::foundation::{non_negative, optional_non_negative, optional_positive};
use crate::lifecycle::cooldown::Cooldown;
use crate::lifecycle::ended::Despawning;
use crate::overlap::hit::DamageDef;
use crate::{TIME_EPSILON, count_down};

/// [`EffectsPlugin`]'s system that counts effects down and lands their
/// periodic payloads, in the tick phase after
/// [`STATS_APPLY`](crate::STATS_APPLY) when it is registered.
pub const EFFECT_TICK: &str = "effect_tick";

/// [`EffectsPlugin`]'s system that applies the queued requests, in the
/// resolve phase after [`DAMAGE_RESOLVE`](crate::DAMAGE_RESOLVE) and the
/// ailment feed when they are registered.
pub const EFFECT_RESOLVE: &str = "effect_resolve";

/// The data kind of effects, and the scene component.
pub(crate) const EFFECTS: &str = "effects";
/// The shapes an effect's fields refer to.
const EFFECT: &str = "effect";
const EFFECT_STACK: &str = "effect_stack";
const EFFECT_SCALE: &str = "effect_scale";
const EFFECT_RESTORE: &str = "effect_restore";
const STAT_MODIFIER: &str = "stat_modifier";
const RESIST_MODIFIER: &str = "resist_modifier";
pub(crate) const MODIFIER: &str = "modifier";

/// The most period elapses one instance lands in a step, as a cooldown
/// bounds its.
const MAX_ELAPSES_PER_STEP: u32 = Cooldown::MAX_ELAPSES_PER_STEP;

/// The names of what a scale reaches, as data writes them.
const INTO_NAMES: &[&str] = &["duration", "periodic", "modifiers"];

/// Whether `name` is a stat name, as `Stats` reads one: `[a-z][a-z0-9_]*`.
fn valid_stat(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// `amount × factor` in `f64`, clamped to `±f32::MAX`.
fn scaled(amount: f32, factor: f64) -> f32 {
    let limit = f64::from(f32::MAX);
    (f64::from(amount) * factor).clamp(-limit, limit) as f32
}

/// How a request on an entity that already carries the effect changes its
/// time left. A match outside this crate needs a wildcard arm, since later
/// policies are additions.
///
/// Data form, snake case: `"refresh"`, `"keep"`, `"extend"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Refresh {
    /// The time left starts over at the duration.
    #[default]
    Refresh,
    /// The time left stays as it is.
    Keep,
    /// The duration is added to the time left.
    Extend,
}

/// What a timed effect does at its end. A match outside this crate needs
/// a wildcard arm, since later policies are additions.
///
/// Data form, snake case: `"clear"`, `"remove_one"`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Expire {
    /// Every stack ends: the instance leaves with its modifiers and tags.
    #[default]
    Clear,
    /// One stack ends and the time left starts over; the last stack ends
    /// as `clear`.
    RemoveOne,
}

/// What an effect's scale factor reaches: its duration, its damage and heal
/// payload, and its `add` and `pct` modifiers, each when named.
///
/// Data form: `["duration", "periodic", "modifiers"]`, all three by
/// default.
///
/// # Examples
///
/// ```
/// use tungsten_kit::Scaled;
///
/// let payload_only = Scaled::NONE.periodic();
/// assert!(payload_only.scales_periodic() && !payload_only.scales_duration());
/// assert_eq!(Scaled::default(), Scaled::ALL);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Scaled {
    duration: bool,
    periodic: bool,
    modifiers: bool,
}

impl Default for Scaled {
    /// [`ALL`](Self::ALL).
    fn default() -> Self {
        Self::ALL
    }
}

impl Scaled {
    /// Every target: the duration, the payload and the modifiers.
    pub const ALL: Self = Self {
        duration: true,
        periodic: true,
        modifiers: true,
    };
    /// No target: the factor changes nothing.
    pub const NONE: Self = Self {
        duration: false,
        periodic: false,
        modifiers: false,
    };

    /// Also the duration.
    #[must_use]
    pub fn duration(mut self) -> Self {
        self.duration = true;
        self
    }

    /// Also the damage and heal payload, periodic or once.
    #[must_use]
    pub fn periodic(mut self) -> Self {
        self.periodic = true;
        self
    }

    /// Also the `add` and `pct` modifiers.
    #[must_use]
    pub fn modifiers(mut self) -> Self {
        self.modifiers = true;
        self
    }

    /// Whether the duration is scaled.
    #[must_use]
    pub fn scales_duration(self) -> bool {
        self.duration
    }

    /// Whether the payload is scaled.
    #[must_use]
    pub fn scales_periodic(self) -> bool {
        self.periodic
    }

    /// Whether the modifiers are scaled.
    #[must_use]
    pub fn scales_modifiers(self) -> bool {
        self.modifiers
    }

    /// The names it holds, as data writes them.
    fn names(self) -> Vec<IntoName> {
        let mut names = Vec::new();
        if self.duration {
            names.push(IntoName::Duration);
        }
        if self.periodic {
            names.push(IntoName::Periodic);
        }
        if self.modifiers {
            names.push(IntoName::Modifiers);
        }
        names
    }
}

/// One target of a scale, as data names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum IntoName {
    Duration,
    Periodic,
    Modifiers,
}

impl FromIterator<IntoName> for Scaled {
    fn from_iter<I: IntoIterator<Item = IntoName>>(names: I) -> Self {
        names.into_iter().fold(Self::NONE, |into, name| match name {
            IntoName::Duration => into.duration(),
            IntoName::Periodic => into.periodic(),
            IntoName::Modifiers => into.modifiers(),
        })
    }
}

/// An effect's definition: what it does to the entity it is applied to,
/// for how long. Timed (`duration` above 0), infinite (none) or instant
/// (0, or a scaled duration of 0): an instant effect lands its payload and
/// queues its `on_expire`, and keeps no instance, so its modifiers and tags
/// do nothing. Its
/// `damage`, periodic ([`with_periodic`](crate::DamageRequest::with_periodic),
/// so it passes the target's invulnerability window and starts none) and
/// its `heal` land at the end of each `period`, or once at the application
/// without one, from the instance's source. Its modifiers go on the
/// target's [`Stats`] and [`Resist`] per stack, under the source
/// `effect#<serial>`, and leave with it; its `grant` tags go into the
/// target's [`Tags`] and leave when no instance grants them and the entity
/// did not carry them before; a target carrying a `blocked_by` tag refuses
/// it. A `restore` raises the base of one of the target's stats with each
/// payload, by its amount times the payload's factor as `heal` takes it, up
/// to the value of the stat its `max` names, at once in the tick, since an
/// ability's cost reads the base; a base at or above that cap is left
/// alone. While the target carries a `suppressed_by` tag, a running instance
/// holds: its time left and its period wait, no payload lands and it cannot
/// end, while its modifiers and tags stay. A `scale` turns the request's scale into a factor through a curve
/// whose every `y` is at least 0, which multiplies what
/// [`Scaled`] names; a timed end applies `on_expire` to the target from
/// the same source.
///
/// Data form, an entry of the `effects` kind or an effect in place; every
/// field optional:
///
/// ```json
/// {
///   "duration": 3, "period": 0.5,
///   "damage": { "elemental.fire": 1 }, "heal": 0,
///   "stack": { "limit": 3, "refresh": "extend", "expire": "remove_one" },
///   "modifiers": [{ "stat": "move_speed", "modifier": { "pct": -0.2 } }],
///   "resist": [{ "tag": "digital", "modifier": { "mul": 2 } }],
///   "grant": ["status.burning"], "blocked_by": ["status.wet"],
///   "restore": { "stat": "mana", "amount": 1.5, "max": "max_mana" },
///   "suppressed_by": ["casting"],
///   "scale": { "curve": "burn_scale", "into": ["duration", "periodic"] },
///   "on_expire": "thawed"
/// }
/// ```
///
/// `damage` a number or declared damage types to amounts; a `restore` names
/// its three fields, its amount above 0; the load checks the damage types,
/// the tags, the curve (every `y` at least 0) and the `on_expire` effect.
///
/// # Examples
///
/// ```
/// use tungsten_core::DataRef;
/// use tungsten_kit::{Effect, Expire, Modifier, Refresh, Scaled};
///
/// let burning = Effect::new()
///     .with_duration(3.0)
///     .with_period(0.5)
///     .with_typed_damage("elemental.fire", 1.0)
///     .with_grant(["status.burning"])
///     .with_stack(3, Refresh::Extend, Expire::RemoveOne)
///     .with_modifier("move_speed", Modifier::Pct(-0.2))
///     .with_scale(DataRef::by_id("burn_scale"), Scaled::NONE.periodic());
/// assert_eq!((burning.duration(), burning.period()), (Some(3.0), Some(0.5)));
/// assert_eq!(burning.damage(), 1.0);
/// assert_eq!(burning.stack_limit(), 3);
/// assert!(!burning.is_instant() && Effect::new().with_duration(0.0).is_instant());
/// let data = serde_json::to_value(&burning).unwrap();
/// assert_eq!(data["grant"], serde_json::json!(["status.burning"]));
/// assert_eq!(data["scale"]["into"], serde_json::json!(["periodic"]));
/// assert_eq!(serde_json::from_value::<Effect>(data).unwrap(), burning);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "EffectDef", into = "EffectDef")]
pub struct Effect {
    duration: Option<f32>,
    period: Option<f32>,
    damage: DamageDef,
    heal: f32,
    limit: u32,
    refresh: Refresh,
    expire: Expire,
    /// In order.
    modifiers: Vec<(String, Modifier)>,
    resist: Vec<(String, Modifier)>,
    /// Sorted and deduplicated.
    grant: Vec<String>,
    blocked_by: Vec<String>,
    restore: Option<Restore>,
    /// Sorted and deduplicated.
    suppressed_by: Vec<String>,
    scale: Option<(DataRef<Curve>, Scaled)>,
    on_expire: Option<DataRef<Effect>>,
}

/// A restore payload: the stat it raises, by how much a payload, and the
/// stat whose value caps it.
#[derive(Debug, Clone, PartialEq)]
struct Restore {
    stat: String,
    amount: f32,
    max: String,
}

impl Default for Effect {
    /// [`new`](Self::new).
    fn default() -> Self {
        Self::new()
    }
}

/// `names` checked as tag names, sorted and deduplicated.
fn tag_list(method: &str, names: impl IntoIterator<Item = impl Into<String>>) -> Vec<String> {
    let mut names: Vec<String> = names.into_iter().map(Into::into).collect();
    for name in &names {
        assert!(
            valid_tag(name),
            "{method}: `{name}` is not a tag name (`[a-z][a-z0-9_]*` segments joined by dots)"
        );
    }
    names.sort();
    names.dedup();
    names
}

impl Effect {
    /// An infinite effect that does nothing: no payload, no modifier, no
    /// tag, one stack.
    #[must_use]
    pub fn new() -> Self {
        Self {
            duration: None,
            period: None,
            damage: DamageDef::Number(0.0),
            heal: 0.0,
            limit: 1,
            refresh: Refresh::Refresh,
            expire: Expire::Clear,
            modifiers: Vec::new(),
            resist: Vec::new(),
            grant: Vec::new(),
            blocked_by: Vec::new(),
            restore: None,
            suppressed_by: Vec::new(),
            scale: None,
            on_expire: None,
        }
    }

    /// The effect, lasting `seconds` of game time; 0 is instant.
    ///
    /// # Panics
    ///
    /// When `seconds` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_duration(mut self, seconds: f32) -> Self {
        assert!(
            seconds.is_finite() && seconds >= 0.0,
            "Effect::with_duration: seconds must be finite and >= 0 (got {seconds})"
        );
        self.duration = Some(seconds);
        self
    }

    /// The effect, landing its payload at the end of each `seconds` of game
    /// time.
    ///
    /// # Panics
    ///
    /// When `seconds` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_period(mut self, seconds: f32) -> Self {
        assert!(
            seconds.is_finite() && seconds > 0.0,
            "Effect::with_period: seconds must be finite and > 0 (got {seconds})"
        );
        self.period = Some(seconds);
        self
    }

    /// The effect, with an untyped payload of `amount`, in place of any
    /// typed one.
    ///
    /// # Panics
    ///
    /// When `amount` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_damage(mut self, amount: f32) -> Self {
        assert!(
            amount.is_finite() && amount >= 0.0,
            "Effect::with_damage: amount must be finite and >= 0 (got {amount})"
        );
        self.damage = DamageDef::Number(amount);
        self
    }

    /// The effect, with `amount` of the damage type `kind` in its payload,
    /// added or replaced; an untyped payload gives way to the typed one. An
    /// amount of 0 is no part: it removes the type, and a payload left with
    /// no type is untyped 0, as the data form has it.
    ///
    /// # Panics
    ///
    /// When `kind` is not a tag name, or `amount` is not finite and at
    /// least 0.
    #[must_use]
    #[track_caller]
    pub fn with_typed_damage(mut self, kind: &str, amount: f32) -> Self {
        assert!(
            valid_tag(kind),
            "Effect::with_typed_damage: `{kind}` is not a damage type (`[a-z][a-z0-9_]*` segments joined by dots)"
        );
        assert!(
            amount.is_finite() && amount >= 0.0,
            "Effect::with_typed_damage: amount must be finite and >= 0 (got {amount})"
        );
        let mut parts = match std::mem::replace(&mut self.damage, DamageDef::Number(0.0)) {
            DamageDef::Typed(parts) => parts,
            DamageDef::Number(_) => BTreeMap::new(),
        };
        if amount > 0.0 {
            parts.insert(kind.to_owned(), amount);
        } else {
            parts.remove(kind);
        }
        self.damage = if parts.is_empty() {
            DamageDef::Number(0.0)
        } else {
            DamageDef::Typed(parts)
        };
        self
    }

    /// The effect, healing `amount` with each payload.
    ///
    /// # Panics
    ///
    /// When `amount` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_heal(mut self, amount: f32) -> Self {
        assert!(
            amount.is_finite() && amount >= 0.0,
            "Effect::with_heal: amount must be finite and >= 0 (got {amount})"
        );
        self.heal = amount;
        self
    }

    /// The effect, stacking up to `limit` times, a request on an entity
    /// that carries it changing its time left by `refresh`, and a timed end
    /// doing `expire`.
    ///
    /// # Panics
    ///
    /// When `limit` is 0.
    #[must_use]
    #[track_caller]
    pub fn with_stack(mut self, limit: u32, refresh: Refresh, expire: Expire) -> Self {
        assert!(limit >= 1, "Effect::with_stack: limit must be at least 1");
        self.limit = limit;
        self.refresh = refresh;
        self.expire = expire;
        self
    }

    /// The effect, putting `modifier` on the target's stat `stat` per
    /// stack, after its earlier modifiers.
    ///
    /// # Panics
    ///
    /// When `stat` is not `[a-z][a-z0-9_]*` or the modifier's value is not
    /// finite.
    #[must_use]
    #[track_caller]
    pub fn with_modifier(mut self, stat: &str, modifier: Modifier) -> Self {
        assert!(
            valid_stat(stat),
            "Effect::with_modifier: `{stat}` is not a stat name (`[a-z][a-z0-9_]*`)"
        );
        assert!(
            modifier.is_finite(),
            "Effect::with_modifier: {stat}: the modifier must be finite (got {modifier:?})"
        );
        self.modifiers.push((stat.to_owned(), modifier));
        self
    }

    /// The effect, putting `modifier` on the target's resistance to the
    /// damage tag `tag` per stack, after its earlier ones.
    ///
    /// # Panics
    ///
    /// When `tag` is not a tag name or the modifier's value is not finite.
    #[must_use]
    #[track_caller]
    pub fn with_resist(mut self, tag: &str, modifier: Modifier) -> Self {
        assert!(
            valid_tag(tag),
            "Effect::with_resist: `{tag}` is not a tag name (`[a-z][a-z0-9_]*` segments joined by dots)"
        );
        assert!(
            modifier.is_finite(),
            "Effect::with_resist: {tag}: the modifier must be finite (got {modifier:?})"
        );
        self.resist.push((tag.to_owned(), modifier));
        self
    }

    /// The effect, also granting `tags` to the target while an instance
    /// runs.
    ///
    /// # Panics
    ///
    /// When a name is not a tag name.
    #[must_use]
    #[track_caller]
    pub fn with_grant(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let mut names = std::mem::take(&mut self.grant);
        names.extend(tags.into_iter().map(Into::into));
        self.grant = tag_list("Effect::with_grant", names);
        self
    }

    /// The effect, also refused by a target that carries any of `tags`.
    ///
    /// # Panics
    ///
    /// When a name is not a tag name.
    #[must_use]
    #[track_caller]
    pub fn with_blocked_by(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let mut names = std::mem::take(&mut self.blocked_by);
        names.extend(tags.into_iter().map(Into::into));
        self.blocked_by = tag_list("Effect::with_blocked_by", names);
        self
    }

    /// The effect, raising the target's base of `stat` by `amount` with each
    /// payload, times the payload's factor, up to the value of the stat
    /// `max`: mana that regenerates to `max_mana`.
    ///
    /// # Panics
    ///
    /// When `stat` or `max` is not `[a-z][a-z0-9_]*`, or `amount` is not
    /// finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_restore(mut self, stat: &str, amount: f32, max: &str) -> Self {
        for name in [stat, max] {
            assert!(
                valid_stat(name),
                "Effect::with_restore: `{name}` is not a stat name (`[a-z][a-z0-9_]*`)"
            );
        }
        assert!(
            amount.is_finite() && amount > 0.0,
            "Effect::with_restore: amount must be finite and > 0 (got {amount})"
        );
        self.restore = Some(Restore {
            stat: stat.to_owned(),
            amount,
            max: max.to_owned(),
        });
        self
    }

    /// The effect, holding a running instance while its target carries any
    /// of `tags`: time and period wait, no payload lands, nothing ends.
    ///
    /// # Panics
    ///
    /// When a name is not a tag name.
    #[must_use]
    #[track_caller]
    pub fn with_suppressed_by(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let mut names = std::mem::take(&mut self.suppressed_by);
        names.extend(tags.into_iter().map(Into::into));
        self.suppressed_by = tag_list("Effect::with_suppressed_by", names);
        self
    }

    /// The effect, scaled: `curve` at the request's scale is the factor,
    /// which multiplies what `into` names. The load refuses a curve with a
    /// `y` below 0.
    #[must_use]
    pub fn with_scale(mut self, curve: DataRef<Curve>, into: Scaled) -> Self {
        self.scale = Some((curve, into));
        self
    }

    /// The effect, applying `effect` to the target from the same source
    /// when a timed instance ends.
    #[must_use]
    pub fn with_on_expire(mut self, effect: DataRef<Effect>) -> Self {
        self.on_expire = Some(effect);
        self
    }

    /// How long it lasts: `None` for an infinite effect, 0 for an instant
    /// one.
    #[must_use]
    pub fn duration(&self) -> Option<f32> {
        self.duration
    }

    /// Whether it lands and ends at the application, at a factor of 1; a
    /// scale that makes the duration 0 does the same.
    #[must_use]
    pub fn is_instant(&self) -> bool {
        self.duration == Some(0.0)
    }

    /// The seconds between its payloads, when it has a period.
    #[must_use]
    pub fn period(&self) -> Option<f32> {
        self.period
    }

    /// Its payload's damage, a typed payload's parts summed.
    #[must_use]
    pub fn damage(&self) -> f32 {
        self.damage.total()
    }

    /// Its payload's heal.
    #[must_use]
    pub fn heal(&self) -> f32 {
        self.heal
    }

    /// The most stacks an instance holds.
    #[must_use]
    pub fn stack_limit(&self) -> u32 {
        self.limit
    }

    /// How a request on an entity that carries it changes its time left.
    #[must_use]
    pub fn refresh(&self) -> Refresh {
        self.refresh
    }

    /// What a timed end does.
    #[must_use]
    pub fn expire(&self) -> Expire {
        self.expire
    }

    /// Its stat modifiers, in order.
    pub fn modifiers(&self) -> impl Iterator<Item = (&str, Modifier)> {
        self.modifiers
            .iter()
            .map(|(stat, modifier)| (stat.as_str(), *modifier))
    }

    /// Its resistance modifiers, in order.
    pub fn resist(&self) -> impl Iterator<Item = (&str, Modifier)> {
        self.resist
            .iter()
            .map(|(tag, modifier)| (tag.as_str(), *modifier))
    }

    /// The tags it grants, sorted.
    #[must_use]
    pub fn grant(&self) -> &[String] {
        &self.grant
    }

    /// The tags that refuse it, sorted.
    #[must_use]
    pub fn blocked_by(&self) -> &[String] {
        &self.blocked_by
    }

    /// Its restore, when it has one: the stat it raises, the amount a
    /// payload and the stat whose value caps it.
    #[must_use]
    pub fn restore(&self) -> Option<(&str, f32, &str)> {
        self.restore
            .as_ref()
            .map(|restore| (restore.stat.as_str(), restore.amount, restore.max.as_str()))
    }

    /// The tags that hold a running instance, sorted.
    #[must_use]
    pub fn suppressed_by(&self) -> &[String] {
        &self.suppressed_by
    }

    /// Its scale curve and what the factor reaches, when it has one.
    #[must_use]
    pub fn scale(&self) -> Option<(&DataRef<Curve>, Scaled)> {
        self.scale.as_ref().map(|(curve, into)| (curve, *into))
    }

    /// The effect a timed end applies, when it has one.
    #[must_use]
    pub fn on_expire(&self) -> Option<&DataRef<Effect>> {
        self.on_expire.as_ref()
    }

    /// Whether the factor reaches `pick`'s target.
    fn scales(&self, pick: fn(Scaled) -> bool) -> bool {
        self.scale.as_ref().is_some_and(|(_, into)| pick(*into))
    }

    /// The duration an instance applied at `factor` takes.
    fn scaled_duration(&self, factor: f32) -> Option<f32> {
        self.duration.map(|duration| {
            if self.scales(Scaled::scales_duration) {
                scaled(duration, f64::from(factor)).max(0.0)
            } else {
                duration
            }
        })
    }

    /// The payload's factor at `factor`.
    fn payload_factor(&self, factor: f32) -> f64 {
        if self.scales(Scaled::scales_periodic) {
            f64::from(factor)
        } else {
            1.0
        }
    }

    /// The damage request of one payload on `target` from `source`, at
    /// `factor`: periodic, with no position, knockback or crit; `None`
    /// when the payload holds no damage.
    fn damage_request(
        &self,
        target: Entity,
        source: Option<Entity>,
        factor: f64,
    ) -> Option<DamageRequest> {
        let mut request = match &self.damage {
            DamageDef::Number(amount) if *amount > 0.0 => {
                DamageRequest::new(target, scaled(*amount, factor))
            }
            DamageDef::Number(_) => return None,
            DamageDef::Typed(parts) => DamageRequest::from_parts(
                target,
                parts.iter().map(|(name, &amount)| {
                    DamagePart::new(scaled(amount, factor)).with_tags(Tags::new([name.as_str()]))
                }),
            ),
        };
        request = request.with_periodic(true);
        if let Some(source) = source {
            request = request.with_source(source);
        }
        Some(request)
    }

    /// The heal request of one payload; `None` when it heals nothing.
    fn heal_request(
        &self,
        target: Entity,
        source: Option<Entity>,
        factor: f64,
    ) -> Option<HealRequest> {
        if self.heal <= 0.0 {
            return None;
        }
        let mut request = HealRequest::new(target, scaled(self.heal, factor));
        if let Some(source) = source {
            request = request.with_source(source);
        }
        Some(request)
    }

    /// The load's check: the payload's types, the tags, the curve and the
    /// `on_expire` effect. A restore's stats are checked as names when it
    /// is read, as the `stats` kind's are; a target that lacks one takes
    /// nothing, as a modifier's stat.
    pub(crate) fn check(&self, world: &World) -> Result<(), MissingAsset> {
        self.damage.check(world)?;
        for (i, tag) in self.grant.iter().enumerate() {
            check_declared(world, &format!("grant.{i}"), tag)?;
        }
        for (i, tag) in self.blocked_by.iter().enumerate() {
            check_declared(world, &format!("blocked_by.{i}"), tag)?;
        }
        for (i, tag) in self.suppressed_by.iter().enumerate() {
            check_declared(world, &format!("suppressed_by.{i}"), tag)?;
        }
        for (i, (tag, _)) in self.resist.iter().enumerate() {
            check_declared(world, &format!("resist.{i}.tag"), tag)?;
        }
        if let Some((curve, _)) = &self.scale {
            curve.check(world, "scale.curve")?;
            if let Some(found) = curve_of(world, curve)
                && found.min_y() < 0.0
            {
                let low = found
                    .points()
                    .iter()
                    .find(|point| point.1 < 0.0)
                    .map_or_else(String::new, |(x, y)| format!("[{x}, {y}]"));
                return Err(MissingAsset::new(
                    "scale.curve",
                    "curve with every y at least 0",
                    low,
                ));
            }
        }
        if let Some(on_expire) = &self.on_expire {
            on_expire.check(world, "on_expire")?;
        }
        Ok(())
    }
}

/// An effect as data writes it, every field optional.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EffectDef {
    #[serde(default, deserialize_with = "optional_non_negative")]
    duration: Option<f32>,
    #[serde(default, deserialize_with = "optional_positive")]
    period: Option<f32>,
    #[serde(default = "no_damage")]
    damage: DamageDef,
    #[serde(default, deserialize_with = "non_negative")]
    heal: f32,
    #[serde(default)]
    stack: StackDef,
    #[serde(default)]
    modifiers: Vec<StatModifierDef>,
    #[serde(default)]
    resist: Vec<ResistModifierDef>,
    #[serde(default)]
    grant: TagList,
    #[serde(default)]
    blocked_by: TagList,
    #[serde(default)]
    restore: Option<RestoreDef>,
    #[serde(default)]
    suppressed_by: TagList,
    #[serde(default)]
    scale: Option<ScaleDef>,
    #[serde(default)]
    on_expire: Option<DataRef<Effect>>,
}

fn no_damage() -> DamageDef {
    DamageDef::Number(0.0)
}

/// A stack policy as data writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StackDef {
    #[serde(default = "one", deserialize_with = "at_least_one")]
    limit: u32,
    #[serde(default)]
    refresh: Refresh,
    #[serde(default)]
    expire: Expire,
}

impl Default for StackDef {
    fn default() -> Self {
        Self {
            limit: 1,
            refresh: Refresh::Refresh,
            expire: Expire::Clear,
        }
    }
}

fn one() -> u32 {
    1
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

/// A stat modifier as data writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "StatModifierFields")]
struct StatModifierDef {
    stat: String,
    modifier: Modifier,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatModifierFields {
    stat: String,
    modifier: Modifier,
}

impl TryFrom<StatModifierFields> for StatModifierDef {
    type Error = String;

    fn try_from(fields: StatModifierFields) -> Result<Self, Self::Error> {
        if !valid_stat(&fields.stat) {
            return Err(format!(
                "stat: `{}` is not a stat name (`[a-z][a-z0-9_]*`)",
                fields.stat
            ));
        }
        if !fields.modifier.is_finite() {
            return Err(format!(
                "modifier: must be finite (got {:?})",
                fields.modifier
            ));
        }
        Ok(Self {
            stat: fields.stat,
            modifier: fields.modifier,
        })
    }
}

/// A resistance modifier as data writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "ResistModifierFields")]
struct ResistModifierDef {
    tag: String,
    modifier: Modifier,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResistModifierFields {
    tag: String,
    modifier: Modifier,
}

impl TryFrom<ResistModifierFields> for ResistModifierDef {
    type Error = String;

    fn try_from(fields: ResistModifierFields) -> Result<Self, Self::Error> {
        if !valid_tag(&fields.tag) {
            return Err(format!(
                "tag: `{}` is not a tag name (`[a-z][a-z0-9_]*` segments joined by dots)",
                fields.tag
            ));
        }
        if !fields.modifier.is_finite() {
            return Err(format!(
                "modifier: must be finite (got {:?})",
                fields.modifier
            ));
        }
        Ok(Self {
            tag: fields.tag,
            modifier: fields.modifier,
        })
    }
}

/// A restore as data writes it, its three fields required.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "RestoreFields")]
struct RestoreDef {
    stat: String,
    amount: f32,
    max: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RestoreFields {
    stat: String,
    amount: f32,
    max: String,
}

impl TryFrom<RestoreFields> for RestoreDef {
    type Error = String;

    fn try_from(fields: RestoreFields) -> Result<Self, Self::Error> {
        for (field, name) in [("stat", &fields.stat), ("max", &fields.max)] {
            if !valid_stat(name) {
                return Err(format!(
                    "{field}: `{name}` is not a stat name (`[a-z][a-z0-9_]*`)"
                ));
            }
        }
        if !(fields.amount.is_finite() && fields.amount > 0.0) {
            return Err(format!(
                "amount: must be finite and > 0 (got {})",
                fields.amount
            ));
        }
        Ok(Self {
            stat: fields.stat,
            amount: fields.amount,
            max: fields.max,
        })
    }
}

/// Tag names as data writes them, each checked, sorted and deduplicated.
#[derive(Default, Serialize, Deserialize)]
#[serde(try_from = "Vec<String>")]
struct TagList(Vec<String>);

impl TryFrom<Vec<String>> for TagList {
    type Error = String;

    fn try_from(mut names: Vec<String>) -> Result<Self, Self::Error> {
        if let Some(name) = names.iter().find(|name| !valid_tag(name)) {
            return Err(format!(
                "`{name}` is not a tag name (`[a-z][a-z0-9_]*` segments joined by dots)"
            ));
        }
        names.sort();
        names.dedup();
        Ok(Self(names))
    }
}

/// A scale as data writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScaleDef {
    curve: DataRef<Curve>,
    #[serde(default = "every_target")]
    into: Vec<IntoName>,
}

fn every_target() -> Vec<IntoName> {
    Scaled::ALL.names()
}

impl TryFrom<EffectDef> for Effect {
    type Error = String;

    fn try_from(def: EffectDef) -> Result<Self, Self::Error> {
        Ok(Self {
            duration: def.duration,
            period: def.period,
            damage: def.damage,
            heal: def.heal,
            limit: def.stack.limit,
            refresh: def.stack.refresh,
            expire: def.stack.expire,
            modifiers: def
                .modifiers
                .into_iter()
                .map(|m| (m.stat, m.modifier))
                .collect(),
            resist: def
                .resist
                .into_iter()
                .map(|m| (m.tag, m.modifier))
                .collect(),
            grant: def.grant.0,
            blocked_by: def.blocked_by.0,
            restore: def.restore.map(|restore| Restore {
                stat: restore.stat,
                amount: restore.amount,
                max: restore.max,
            }),
            suppressed_by: def.suppressed_by.0,
            scale: def
                .scale
                .map(|scale| (scale.curve, scale.into.into_iter().collect())),
            on_expire: def.on_expire,
        })
    }
}

impl From<Effect> for EffectDef {
    fn from(effect: Effect) -> Self {
        Self {
            duration: effect.duration,
            period: effect.period,
            damage: effect.damage,
            heal: effect.heal,
            stack: StackDef {
                limit: effect.limit,
                refresh: effect.refresh,
                expire: effect.expire,
            },
            modifiers: effect
                .modifiers
                .into_iter()
                .map(|(stat, modifier)| StatModifierDef { stat, modifier })
                .collect(),
            resist: effect
                .resist
                .into_iter()
                .map(|(tag, modifier)| ResistModifierDef { tag, modifier })
                .collect(),
            grant: TagList(effect.grant),
            blocked_by: TagList(effect.blocked_by),
            restore: effect.restore.map(|restore| RestoreDef {
                stat: restore.stat,
                amount: restore.amount,
                max: restore.max,
            }),
            suppressed_by: TagList(effect.suppressed_by),
            scale: effect.scale.map(|(curve, into)| ScaleDef {
                curve,
                into: into.names(),
            }),
            on_expire: effect.on_expire,
        }
    }
}

/// One effect an entity carries: which, from whom, how many stacks, the
/// time left and the factor it was applied at. Built by the kit only.
///
/// # Examples
///
/// ```
/// use tungsten_core::{DataRef, Plugin, Schedule, World};
/// use tungsten_kit::{Effect, EffectRequest, Effects, EffectsPlugin, Applied};
///
/// let mut world = World::new();
/// let mut schedule = Schedule::new();
/// EffectsPlugin.build(&mut schedule, &mut world);
/// let slime = world.spawn();
/// let slow = DataRef::inline(Effect::new().with_duration(2.0));
/// assert_eq!(EffectRequest::new(slime, slow.clone()).apply(&mut world), Applied::Applied);
/// let instance = world.get::<Effects>(slime).unwrap().instances().next().unwrap();
/// assert_eq!((instance.effect(), instance.stacks(), instance.left()), (&slow, 1, Some(2.0)));
/// assert_eq!((instance.source(), instance.serial()), (None, 0));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "InstanceFields", into = "InstanceFields")]
pub struct EffectInstance {
    effect: DataRef<Effect>,
    /// Not saved: an entity is not data.
    source: Option<Entity>,
    stacks: u32,
    /// `None` for an infinite effect.
    left: Option<f32>,
    /// Game seconds to the next payload, when the effect has a period.
    period_left: Option<f32>,
    factor: f32,
    serial: u64,
    /// The tags it granted, in the definition's order.
    granted: Vec<String>,
}

impl EffectInstance {
    /// The effect: its entry's ID, or the definition in place.
    #[must_use]
    pub fn effect(&self) -> &DataRef<Effect> {
        &self.effect
    }

    /// The entity it came from, when the request named one. A save holds
    /// no entity, so it is `None` after a load until the game names it again
    /// with [`Effects::set_source`].
    #[must_use]
    pub fn source(&self) -> Option<Entity> {
        self.source
    }

    /// Its stacks, at least 1.
    #[must_use]
    pub fn stacks(&self) -> u32 {
        self.stacks
    }

    /// The game seconds left; `None` for an infinite effect.
    #[must_use]
    pub fn left(&self) -> Option<f32> {
        self.left
    }

    /// The factor it was applied at: its scale curve at the request's
    /// scale, 1 without one.
    #[must_use]
    pub fn factor(&self) -> f32 {
        self.factor
    }

    /// Its number among the entity's instances, from 0: the source its
    /// modifiers are under, `effect#<serial>`.
    #[must_use]
    pub fn serial(&self) -> u64 {
        self.serial
    }

    /// The modifier source of the instance numbered `serial`.
    fn source_of(serial: u64) -> String {
        format!("effect#{serial}")
    }
}

/// An instance as a save writes it, before its values are checked.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InstanceFields {
    effect: DataRef<Effect>,
    #[serde(default = "one")]
    stacks: u32,
    #[serde(default)]
    left: Option<f32>,
    #[serde(default)]
    period_left: Option<f32>,
    #[serde(default = "one_f32")]
    factor: f32,
    #[serde(default)]
    serial: u64,
    #[serde(default)]
    granted: Vec<String>,
}

fn one_f32() -> f32 {
    1.0
}

impl TryFrom<InstanceFields> for EffectInstance {
    type Error = String;

    fn try_from(fields: InstanceFields) -> Result<Self, Self::Error> {
        if fields.stacks < 1 {
            return Err("stacks: must be at least 1".to_string());
        }
        for (field, value) in [
            ("left", fields.left),
            ("period_left", fields.period_left),
            ("factor", Some(fields.factor)),
        ] {
            if let Some(value) = value
                && !(value.is_finite() && value >= 0.0)
            {
                return Err(format!("{field}: must be finite and >= 0 (got {value})"));
            }
        }
        if let Some(tag) = fields.granted.iter().find(|tag| !valid_tag(tag)) {
            return Err(format!(
                "granted: `{tag}` is not a tag name (`[a-z][a-z0-9_]*` segments joined by dots)"
            ));
        }
        Ok(Self {
            effect: fields.effect,
            source: None,
            stacks: fields.stacks,
            left: fields.left,
            period_left: fields.period_left,
            factor: fields.factor,
            serial: fields.serial,
            granted: fields.granted,
        })
    }
}

impl From<EffectInstance> for InstanceFields {
    fn from(instance: EffectInstance) -> Self {
        Self {
            effect: instance.effect,
            stacks: instance.stacks,
            left: instance.left,
            period_left: instance.period_left,
            factor: instance.factor,
            serial: instance.serial,
            granted: instance.granted,
        }
    }
}

/// One granted tag's ownership: how many live instances grant it, and
/// whether the entity carried it before the first did.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Granted {
    tag: String,
    owners: u32,
    prior: bool,
}

/// The effects an entity carries: the entries to apply at its first
/// [`EFFECT_TICK`] with no source, and the active instances with the tags
/// they granted. [`EFFECT_RESOLVE`] inserts one on a target that lacks it.
/// It is data: a save holds the pending list, the instances and the
/// granted-tag table; an instance's source is not saved, since an entity is
/// not data, and a game's load names it again with
/// [`set_source`](Self::set_source).
///
/// Scene name `effects`: `["regen", { "duration": 2, "grant": ["status.blessed"] }]`,
/// each an `effects` entry's ID or an effect in place; the load checks
/// each.
///
/// # Examples
///
/// ```
/// use tungsten_core::{DataRef, Plugin, Schedule, Stage, Time, World};
/// use tungsten_kit::{Effect, Effects, EffectsPlugin, Modifier, Stats, StatsPlugin};
///
/// let mut world = World::new();
/// world.insert_resource(Time::new());
/// let mut schedule = Schedule::new();
/// StatsPlugin::new().build(&mut schedule, &mut world);
/// EffectsPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// // A slow listed on the prefab: applied at the first step's resolve, so the
/// // stat shows it from that step and a mover follows it for the next two,
/// // and gone at the third step's tick.
/// let slow = Effect::new().with_duration(2.0 / 60.0).with_modifier("move_speed", Modifier::Pct(-0.5));
/// let slime = world.spawn_with((
///     Stats::new().with(Stats::MOVE_SPEED, 100.0),
///     Effects::new().with(DataRef::inline(slow)),
/// ));
/// let mut speeds = Vec::new();
/// for _ in 0..4 {
///     world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
///     schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
///     speeds.push(world.get::<Stats>(slime).unwrap().get("move_speed").unwrap());
/// }
/// assert_eq!(speeds, [50.0, 50.0, 100.0, 100.0]);
/// assert_eq!(world.get::<Effects>(slime).unwrap().instances().count(), 0);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "EffectsSave", into = "EffectsSave")]
pub struct Effects {
    pending: Vec<DataRef<Effect>>,
    instances: Vec<EffectInstance>,
    next_serial: u64,
    /// Sorted by tag.
    granted: Vec<Granted>,
}

/// The component as a save writes it.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct EffectsSave {
    pending: Vec<DataRef<Effect>>,
    instances: Vec<EffectInstance>,
    next_serial: u64,
    granted: Vec<Granted>,
}

impl TryFrom<EffectsSave> for Effects {
    type Error = String;

    fn try_from(mut save: EffectsSave) -> Result<Self, Self::Error> {
        if let Some(row) = save.granted.iter().find(|row| !valid_tag(&row.tag)) {
            return Err(format!(
                "granted: `{}` is not a tag name (`[a-z][a-z0-9_]*` segments joined by dots)",
                row.tag
            ));
        }
        save.granted.sort_by(|a, b| a.tag.cmp(&b.tag));
        Ok(Self {
            pending: save.pending,
            instances: save.instances,
            next_serial: save.next_serial,
            granted: save.granted,
        })
    }
}

impl From<Effects> for EffectsSave {
    fn from(effects: Effects) -> Self {
        Self {
            pending: effects.pending,
            instances: effects.instances,
            next_serial: effects.next_serial,
            granted: effects.granted,
        }
    }
}

impl Effects {
    /// No effects.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The effects, with `effect` applied at the entity's first
    /// [`EFFECT_TICK`] with no source.
    #[must_use]
    pub fn with(mut self, effect: DataRef<Effect>) -> Self {
        self.pending.push(effect);
        self
    }

    /// The entries still to apply.
    #[must_use]
    pub fn pending(&self) -> &[DataRef<Effect>] {
        &self.pending
    }

    /// The active instances, oldest first.
    pub fn instances(&self) -> impl Iterator<Item = &EffectInstance> {
        self.instances.iter()
    }

    /// Whether an instance of the entry `id` is active.
    #[must_use]
    pub fn has(&self, id: &str) -> bool {
        self.instances
            .iter()
            .any(|instance| instance.effect.id() == Some(id))
    }

    /// The stacks of the entry `id`'s instance; 0 without one.
    #[must_use]
    pub fn stacks(&self, id: &str) -> u32 {
        self.instances
            .iter()
            .find(|instance| instance.effect.id() == Some(id))
            .map_or(0, |instance| instance.stacks)
    }

    /// The tags the instances grant, with their live granters.
    pub fn granted(&self) -> impl Iterator<Item = (&str, u32)> {
        self.granted
            .iter()
            .map(|row| (row.tag.as_str(), row.owners))
    }

    /// Names `source` as the source of the active instance numbered
    /// `serial` ([`EffectInstance::serial`]); `None` clears it. A save holds
    /// no entity, so a game's load names each source again this way, from
    /// the game's own ID for it, before the next step: the instance's later
    /// payloads and its `on_expire` then come from `source`. Nothing else
    /// changes: stacks, time, factor, modifiers and granted tags stay.
    /// False, changing nothing, when no active instance has that serial.
    ///
    /// # Examples
    ///
    /// ```
    /// use tungsten_core::{DataRef, Plugin, Schedule, World};
    /// use tungsten_kit::{Effect, EffectRequest, Effects, EffectsPlugin};
    ///
    /// let mut world = World::new();
    /// let mut schedule = Schedule::new();
    /// EffectsPlugin.build(&mut schedule, &mut world);
    /// let (mage, golem) = (world.spawn(), world.spawn());
    /// let burn = DataRef::inline(Effect::new().with_duration(4.0).with_period(0.5).with_damage(1.0));
    /// EffectRequest::new(golem, burn).with_source(mage).apply(&mut world);
    ///
    /// // A save writes no source; the game keeps its own ID for the mage.
    /// let saved = serde_json::to_value(world.get::<Effects>(golem).unwrap()).unwrap();
    /// let mut loaded: Effects = serde_json::from_value(saved).unwrap();
    /// let serial = loaded.instances().next().unwrap().serial();
    /// assert_eq!(loaded.instances().next().unwrap().source(), None);
    /// assert!(loaded.set_source(serial, Some(mage)));
    /// assert_eq!(loaded.instances().next().unwrap().source(), Some(mage));
    /// assert!(!loaded.set_source(serial + 1, Some(mage)), "no such instance");
    /// ```
    pub fn set_source(&mut self, serial: u64, source: Option<Entity>) -> bool {
        match self
            .instances
            .iter_mut()
            .find(|instance| instance.serial == serial)
        {
            Some(instance) => {
                instance.source = source;
                true
            }
            None => false,
        }
    }

    fn find(&self, effect: &DataRef<Effect>) -> Option<usize> {
        self.instances
            .iter()
            .position(|instance| instance.effect == *effect)
    }
}

/// `components.effects` as a scene writes it: the entries to apply.
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct EffectsDef(Vec<DataRef<Effect>>);

impl EffectsDef {
    fn check(&self, world: &World) -> Result<(), MissingAsset> {
        for (i, effect) in self.0.iter().enumerate() {
            effect.check(world, &i.to_string())?;
        }
        Ok(())
    }
}

/// What applying an effect did. A match outside this crate needs a
/// wildcard arm, since later outcomes are additions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Applied {
    /// A new instance, or an instant effect's payload.
    Applied,
    /// An instance already there took the request: one more stack when
    /// below the limit, its time left by the effect's `refresh`.
    Stacked,
    /// The target carries a `blocked_by` tag.
    Blocked,
    /// The target is gone, despawning or dead, or the effect's entry is
    /// missing.
    Ignored,
}

/// An effect to apply: its target, the effect, the entity it comes from and
/// the scale its curve reads. The kit's cues, ladder, abilities and offers
/// queue one for the step's [`EFFECT_RESOLVE`]; a game's Rust builds its
/// own and either [`apply`](Self::apply)s it now or
/// [`queue`](Self::queue)s it.
///
/// Resolving one, now or queued: a target that is gone, despawning or dead
/// is [`Applied::Ignored`]; one carrying a `blocked_by` tag is
/// [`Applied::Blocked`], and the trace says so. The factor is the scale
/// curve at the request's scale, 1 without one. An instant effect lands its
/// payload at once, sends [`EffectApplied`] and [`EffectExpired`], and
/// queues its `on_expire` for the next resolve. Otherwise the target takes
/// an [`Effects`] if it lacks one; with no instance of the effect, one is
/// made with the next serial, one stack, its scaled duration, its modifiers
/// on the target's [`Stats`] and [`Resist`] under `effect#<serial>` and its
/// tags granted, and the payload lands once when the effect has no period
/// ([`Applied::Applied`]); with one, it gains a stack below the limit, its
/// time left follows `refresh`, its factor becomes the request's and its
/// modifiers are rebuilt per stack ([`Applied::Stacked`]). [`EffectApplied`]
/// goes out with the stacks.
///
/// # Examples
///
/// ```
/// use tungsten_core::{DataRef, EventQueue, Plugin, Schedule, World};
/// use tungsten_kit::{
///     Applied, Effect, EffectApplied, EffectRequest, Effects, EffectsPlugin, Modifier, Stats,
///     Tags, TagsPlugin, end_effect,
/// };
///
/// let mut world = World::new();
/// let mut schedule = Schedule::new();
/// TagsPlugin::new().with_tags(["status.chilled", "status.warm"]).build(&mut schedule, &mut world);
/// EffectsPlugin.build(&mut schedule, &mut world);
/// let mage = world.spawn();
/// let golem = world.spawn_with((Stats::new().with(Stats::MOVE_SPEED, 80.0),));
/// let chilled = DataRef::inline(
///     Effect::new()
///         .with_duration(4.0)
///         .with_modifier("move_speed", Modifier::Pct(-0.5))
///         .with_grant(["status.chilled"])
///         .with_blocked_by(["status.warm"]),
/// );
/// let request = EffectRequest::new(golem, chilled.clone()).with_source(mage);
/// assert_eq!(request.clone().apply(&mut world), Applied::Applied);
/// assert_eq!(world.get::<Stats>(golem).unwrap().get("move_speed"), Some(40.0));
/// assert!(world.get::<Tags>(golem).unwrap().has("status.chilled"));
/// assert_eq!(request.clone().apply(&mut world), Applied::Stacked, "one stack, refreshed");
/// let sent = world.resource::<EventQueue<EffectApplied>>().iter().count();
/// assert_eq!(sent, 2);
///
/// assert!(end_effect(&mut world, golem, &chilled));
/// assert_eq!(world.get::<Stats>(golem).unwrap().get("move_speed"), Some(80.0));
/// assert!(!world.get::<Tags>(golem).unwrap().has("status.chilled"));
///
/// world.get_mut::<Tags>(golem).unwrap().insert("status.warm");
/// assert_eq!(request.apply(&mut world), Applied::Blocked);
/// assert!(world.get::<Effects>(golem).unwrap().instances().next().is_none());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct EffectRequest {
    target: Entity,
    effect: DataRef<Effect>,
    source: Option<Entity>,
    scale: f32,
}

impl EffectRequest {
    /// A request of `effect` on `target`, from no source, at scale 1.
    #[must_use]
    pub fn new(target: Entity, effect: DataRef<Effect>) -> Self {
        Self {
            target,
            effect,
            source: None,
            scale: 1.0,
        }
    }

    /// The request, from `source`: the instance's source, which its
    /// payloads and its `on_expire` come from, and the `effect_applied` cue
    /// event's other entity.
    #[must_use]
    pub fn with_source(mut self, source: Entity) -> Self {
        self.source = Some(source);
        self
    }

    /// The request, at `scale`: what the effect's curve reads for its
    /// factor.
    ///
    /// # Panics
    ///
    /// When `scale` is not finite.
    #[must_use]
    #[track_caller]
    pub fn with_scale(mut self, scale: f32) -> Self {
        assert!(
            scale.is_finite(),
            "EffectRequest::with_scale: scale must be finite (got {scale})"
        );
        self.scale = scale;
        self
    }

    /// The entity the effect goes on.
    #[must_use]
    pub fn target(&self) -> Entity {
        self.target
    }

    /// The effect.
    #[must_use]
    pub fn effect(&self) -> &DataRef<Effect> {
        &self.effect
    }

    /// The entity it comes from, when it has one.
    #[must_use]
    pub fn source(&self) -> Option<Entity> {
        self.source
    }

    /// The scale its curve reads.
    #[must_use]
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// Applies the effect now, so the caller knows the outcome in the same
    /// step: the game's immediate form, since every kit path queues.
    pub fn apply(self, world: &mut World) -> Applied {
        resolve_effect(world, &self)
    }

    /// Queues the request for [`EFFECT_RESOLVE`], this step's when it has
    /// not run yet, else the next step's. False, queueing nothing, when the
    /// World has no [`EffectsPlugin`] queue.
    pub fn queue(self, world: &mut World) -> bool {
        queue_request(world, self, false)
    }
}

/// Ends `effect`'s instance on `entity` early: its modifiers and tags
/// leave, [`EffectExpired`] goes out with `ended` set, and its `on_expire`
/// does not apply. Whether there was one.
pub fn end_effect(world: &mut World, entity: Entity, effect: &DataRef<Effect>) -> bool {
    let Some(slot) = world.get_mut::<Effects>(entity) else {
        return false;
    };
    let Some(at) = slot.find(effect) else {
        return false;
    };
    let mut effects = std::mem::take(slot);
    let instance = effects.instances.remove(at);
    release(world, entity, &mut effects, &instance);
    put_back(world, entity, effects);
    send_expired(world, entity, instance.effect.id().map(str::to_owned), true);
    true
}

/// An effect applied or stacked, sent once [`EffectsPlugin`] has registered
/// its queue: the cue event `effect_applied`, whose other entity is the
/// source and whose amount is the stacks. Build one with
/// [`EffectApplied::new`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct EffectApplied {
    /// The entity the effect went on.
    pub entity: Entity,
    /// The entity it came from, when the request named one.
    pub source: Option<Entity>,
    /// The instance's stacks after the request; 1 for an instant effect.
    pub stacks: u32,
    /// The effect's entry ID; `None` for an effect written in place.
    pub id: Option<String>,
}

impl EffectApplied {
    /// An event with these fields, no source and no ID.
    #[must_use]
    pub fn new(entity: Entity, stacks: u32) -> Self {
        Self {
            entity,
            source: None,
            stacks,
            id: None,
        }
    }

    /// The event, from `source`.
    #[must_use]
    pub fn with_source(mut self, source: Entity) -> Self {
        self.source = Some(source);
        self
    }

    /// The event, of the entry `id`.
    #[must_use]
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }
}

/// An effect's instance ended, sent once [`EffectsPlugin`] has registered
/// its queue: the cue event `effect_expired`. Build one with
/// [`EffectExpired::new`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct EffectExpired {
    /// The entity the effect was on.
    pub entity: Entity,
    /// The effect's entry ID; `None` for an effect written in place.
    pub id: Option<String>,
    /// [`end_effect`] ended it, before its time.
    pub ended: bool,
}

impl EffectExpired {
    /// An event with these fields and no ID.
    #[must_use]
    pub fn new(entity: Entity, ended: bool) -> Self {
        Self {
            entity,
            id: None,
            ended,
        }
    }

    /// The event, of the entry `id`.
    #[must_use]
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }
}

/// The queued requests, a World resource the plugin inserts: the step's
/// `on_expire` transitions, which resolve first, and the rest.
#[derive(Debug, Default)]
pub(crate) struct EffectRequests {
    expiries: Vec<EffectRequest>,
    requests: Vec<EffectRequest>,
}

/// Queues `request`, in the expiries list when `expiry`; whether the World
/// has the queue.
fn queue_request(world: &mut World, request: EffectRequest, expiry: bool) -> bool {
    match world.get_resource_mut::<EffectRequests>() {
        Some(requests) => {
            if expiry {
                requests.expiries.push(request);
            } else {
                requests.requests.push(request);
            }
            true
        }
        None => false,
    }
}

/// Why a request or an instance's work did not run, for the trace.
const GONE: &str = "the target is gone, despawning or dead";
const NO_EFFECT: &str = "the World's data has no such effect";
const BLOCKED: &str = "the target carries a blocking tag";
const NO_STATS: &str = "the target has no Stats for the modifiers";
const NO_RESIST: &str = "the target has no Resist for the modifiers";
const NO_RESTORE: &str = "the target has no Stats with the restore's stat and its max";
const SUPPRESSED: &str = "the target carries a suppressing tag";

/// Traces why `request`'s work on `target` did not run.
fn skip(world: &mut World, target: Entity, action: &'static str, why: &'static str) {
    trace(world, target, || TraceKind::Skipped {
        event: "effect",
        action,
        why,
    });
}

/// `effect`'s definition: the one in place, or the entry the World's data
/// holds now.
pub(crate) fn definition<'a>(world: &'a World, effect: &'a DataRef<Effect>) -> Option<&'a Effect> {
    match world.get_resource::<DataRegistry>() {
        Some(data) => effect.resolve(data),
        None => effect.as_inline(),
    }
}

/// `curve`'s definition, as [`definition`] finds an effect's.
fn curve_of<'a>(world: &'a World, curve: &'a DataRef<Curve>) -> Option<&'a Curve> {
    match world.get_resource::<DataRegistry>() {
        Some(data) => curve.resolve(data),
        None => curve.as_inline(),
    }
}

/// The factor `effect` takes at `scale`: its curve there, 1 without a
/// curve, never below 0.
fn factor_of(world: &World, effect: &Effect, scale: f32) -> f32 {
    effect.scale.as_ref().map_or(1.0, |(curve, _)| {
        curve_of(world, curve).map_or(1.0, |curve| curve.at(scale).max(0.0))
    })
}

/// Puts `effects` back on `entity`.
fn put_back(world: &mut World, entity: Entity, effects: Effects) {
    match world.get_mut::<Effects>(entity) {
        Some(slot) => *slot = effects,
        None => world.insert(entity, effects),
    }
}

/// Sends [`EffectApplied`] and traces it.
fn send_applied(
    world: &mut World,
    entity: Entity,
    source: Option<Entity>,
    stacks: u32,
    id: Option<String>,
) {
    if let Some(queue) = world.get_resource_mut::<EventQueue<EffectApplied>>() {
        queue.send(EffectApplied {
            entity,
            source,
            stacks,
            id,
        });
    }
    trace(world, entity, || TraceKind::Event {
        name: "effect_applied",
        other: source,
        amount: Some(stacks as f32),
    });
}

/// Sends [`EffectExpired`] and traces it.
fn send_expired(world: &mut World, entity: Entity, id: Option<String>, ended: bool) {
    if let Some(queue) = world.get_resource_mut::<EventQueue<EffectExpired>>() {
        queue.send(EffectExpired { entity, id, ended });
    }
    trace(world, entity, || TraceKind::Event {
        name: "effect_expired",
        other: None,
        amount: None,
    });
}

/// Lands one payload of `effect` on `target` from `source` at once, at
/// `factor`.
fn land_payload(
    world: &mut World,
    effect: &Effect,
    target: Entity,
    source: Option<Entity>,
    factor: f32,
) {
    let factor = effect.payload_factor(factor);
    if let Some(request) = effect.damage_request(target, source, factor) {
        request.apply(world);
    }
    if let Some(request) = effect.heal_request(target, source, factor) {
        request.apply(world);
    }
    if let Some(restore) = &effect.restore {
        restore_stat(world, target, restore, factor);
    }
}

/// Raises `target`'s base of the restore's stat by its amount times
/// `factor`, up to the value of its `max` stat; a base at or above that is
/// left alone, and a target without [`Stats`] or either stat takes nothing,
/// traced.
fn restore_stat(world: &mut World, target: Entity, restore: &Restore, factor: f64) {
    let Some(stats) = world.get_mut::<Stats>(target) else {
        skip(world, target, "restore", NO_RESTORE);
        return;
    };
    let (Some(base), Some(cap)) = (stats.base(&restore.stat), stats.get(&restore.max)) else {
        skip(world, target, "restore", NO_RESTORE);
        return;
    };
    if base >= cap {
        return;
    }
    let raised = (f64::from(base) + f64::from(restore.amount) * factor)
        .min(f64::from(cap))
        .min(f64::from(f32::MAX));
    stats.set_base(&restore.stat, raised as f32);
}

/// Lands `count` of `instance`'s restore payloads on `target` at once, when
/// its effect has a restore.
fn restore_instance(world: &mut World, target: Entity, instance: &EffectInstance, count: u32) {
    let Some((restore, factor)) = definition(world, &instance.effect).and_then(|effect| {
        Some((
            effect.restore.clone()?,
            effect.payload_factor(instance.factor),
        ))
    }) else {
        return;
    };
    restore_stat(world, target, &restore, factor * f64::from(count));
}

/// Whether `entity` carries a tag that holds `instance`'s effect.
fn suppressed(world: &World, entity: Entity, instance: &EffectInstance) -> bool {
    definition(world, &instance.effect).is_some_and(|effect| {
        !effect.suppressed_by.is_empty()
            && world
                .get::<Tags>(entity)
                .is_some_and(|tags| effect.suppressed_by.iter().any(|tag| tags.has(tag)))
    })
}

/// Puts `effect`'s modifiers on `target` under the instance's source,
/// `stacks` copies each, `add` and `pct` values times the factor when the
/// scale names the modifiers; a target without [`Stats`] or [`Resist`]
/// takes none, traced when `fresh`.
fn place_modifiers(
    world: &mut World,
    target: Entity,
    effect: &Effect,
    instance: &EffectInstance,
    fresh: bool,
) {
    let source = EffectInstance::source_of(instance.serial);
    let factor = if effect.scales(Scaled::scales_modifiers) {
        f64::from(instance.factor)
    } else {
        1.0
    };
    let scale = |modifier: Modifier| match modifier {
        Modifier::Add(v) => Modifier::Add(scaled(v, factor)),
        Modifier::Pct(v) => Modifier::Pct(scaled(v, factor)),
        other => other,
    };
    if !effect.modifiers.is_empty() {
        match world.get_mut::<Stats>(target) {
            Some(stats) => {
                for _ in 0..instance.stacks {
                    for (stat, modifier) in &effect.modifiers {
                        stats.add_modifier(stat, &source, scale(*modifier));
                    }
                }
            }
            None if fresh => skip(world, target, "modifiers", NO_STATS),
            None => {}
        }
    }
    if !effect.resist.is_empty() {
        match world.get_mut::<Resist>(target) {
            Some(resist) => {
                for _ in 0..instance.stacks {
                    for (tag, modifier) in &effect.resist {
                        resist.add_modifier(tag, &source, scale(*modifier));
                    }
                }
            }
            None if fresh => skip(world, target, "resist", NO_RESIST),
            None => {}
        }
    }
}

/// Takes the instance numbered `serial`'s modifiers off `target`.
fn remove_modifiers(world: &mut World, target: Entity, serial: u64) {
    let source = EffectInstance::source_of(serial);
    if let Some(stats) = world.get_mut::<Stats>(target) {
        stats.remove_modifiers(&source);
    }
    if let Some(resist) = world.get_mut::<Resist>(target) {
        resist.remove_modifiers(&source);
    }
}

/// Grants `tags` to `target` for `instance`: each into the entity's
/// [`Tags`], inserted if absent, onto the instance's list and into the
/// component's ownership table.
fn grant(
    world: &mut World,
    target: Entity,
    effects: &mut Effects,
    instance: &mut EffectInstance,
    tags: &[String],
) {
    if tags.is_empty() {
        return;
    }
    if !world.has::<Tags>(target) {
        world.insert(target, Tags::default());
    }
    let carried = world.get_mut::<Tags>(target).unwrap();
    for tag in tags {
        match effects
            .granted
            .binary_search_by(|row| row.tag.as_str().cmp(tag))
        {
            Ok(at) => effects.granted[at].owners += 1,
            Err(at) => {
                let prior = carried.iter().any(|name| name == tag);
                effects.granted.insert(
                    at,
                    Granted {
                        tag: tag.clone(),
                        owners: 1,
                        prior,
                    },
                );
            }
        }
        carried.insert(tag.clone());
        instance.granted.push(tag.clone());
    }
}

/// Takes `instance` off `target`: its modifiers by source, and each tag on
/// its granted list, which leaves the entity when no instance grants it any
/// more and the entity did not carry it before.
fn release(world: &mut World, target: Entity, effects: &mut Effects, instance: &EffectInstance) {
    remove_modifiers(world, target, instance.serial);
    for tag in &instance.granted {
        let Ok(at) = effects
            .granted
            .binary_search_by(|row| row.tag.as_str().cmp(tag))
        else {
            continue;
        };
        let row = &mut effects.granted[at];
        row.owners = row.owners.saturating_sub(1);
        if row.owners > 0 {
            continue;
        }
        let prior = row.prior;
        effects.granted.remove(at);
        if !prior && let Some(carried) = world.get_mut::<Tags>(target) {
            carried.remove(tag);
        }
    }
}

/// [`EffectRequest`]'s resolution.
fn resolve_effect(world: &mut World, request: &EffectRequest) -> Applied {
    let target = request.target;
    let gone = !world.is_alive(target)
        || world.has::<Despawning>(target)
        || world.get::<Health>(target).is_some_and(Health::is_dead);
    if gone {
        skip(world, target, "apply", GONE);
        return Applied::Ignored;
    }
    let Some(effect) = definition(world, &request.effect).cloned() else {
        skip(world, target, "apply", NO_EFFECT);
        return Applied::Ignored;
    };
    if !effect.blocked_by.is_empty()
        && let Some(carried) = world.get::<Tags>(target)
        && effect.blocked_by.iter().any(|tag| carried.has(tag))
    {
        skip(world, target, "apply", BLOCKED);
        return Applied::Blocked;
    }
    let factor = factor_of(world, &effect, request.scale);
    let id = request.effect.id().map(str::to_owned);
    if effect.scaled_duration(factor) == Some(0.0) {
        send_applied(world, target, request.source, 1, id.clone());
        land_payload(world, &effect, target, request.source, factor);
        send_expired(world, target, id, false);
        if let Some(on_expire) = &effect.on_expire {
            let mut next = EffectRequest::new(target, on_expire.clone());
            if let Some(source) = request.source {
                next = next.with_source(source);
            }
            queue_request(world, next, false);
        }
        return Applied::Applied;
    }
    if !world.has::<Effects>(target) {
        world.insert(target, Effects::new());
    }
    let mut effects = std::mem::take(world.get_mut::<Effects>(target).unwrap());
    let (outcome, stacks) = match effects.find(&request.effect) {
        None => {
            let serial = effects.next_serial;
            effects.next_serial += 1;
            let mut instance = EffectInstance {
                effect: request.effect.clone(),
                source: request.source,
                stacks: 1,
                left: effect.scaled_duration(factor),
                period_left: effect.period,
                factor,
                serial,
                granted: Vec::new(),
            };
            place_modifiers(world, target, &effect, &instance, true);
            grant(world, target, &mut effects, &mut instance, &effect.grant);
            effects.instances.push(instance);
            (Applied::Applied, 1)
        }
        Some(at) => {
            let instance = &mut effects.instances[at];
            if instance.stacks < effect.limit {
                instance.stacks += 1;
            }
            let duration = effect.scaled_duration(factor);
            instance.left = match effect.refresh {
                Refresh::Refresh => duration,
                Refresh::Keep => instance.left,
                Refresh::Extend => match (instance.left, duration) {
                    (Some(left), Some(duration)) => Some(
                        (f64::from(left) + f64::from(duration)).min(f64::from(f32::MAX)) as f32,
                    ),
                    _ => None,
                },
            };
            instance.factor = factor;
            if request.source.is_some() {
                instance.source = request.source;
            }
            remove_modifiers(world, target, instance.serial);
            let instance = &effects.instances[at];
            place_modifiers(world, target, &effect, instance, false);
            (Applied::Stacked, instance.stacks)
        }
    };
    put_back(world, target, effects);
    send_applied(world, target, request.source, stacks, id);
    if outcome == Applied::Applied && effect.period.is_none() {
        land_payload(world, &effect, target, request.source, factor);
    }
    outcome
}

/// [`EFFECT_RESOLVE`]'s state between steps: the drained lists, kept so a
/// step allocates nothing once warm.
#[derive(Default)]
struct EffectResolver {
    expiries: Vec<EffectRequest>,
    requests: Vec<EffectRequest>,
}

impl EffectResolver {
    /// Resolves the step's `on_expire` transitions in the order the tick
    /// queued them, then the other requests stable-sorted by target index
    /// and then source index with none first.
    fn run(&mut self, world: &mut World) {
        let Some(requests) = world.get_resource_mut::<EffectRequests>() else {
            return;
        };
        if requests.expiries.is_empty() && requests.requests.is_empty() {
            return;
        }
        std::mem::swap(&mut self.expiries, &mut requests.expiries);
        std::mem::swap(&mut self.requests, &mut requests.requests);
        for request in self.expiries.drain(..) {
            resolve_effect(world, &request);
        }
        self.requests
            .sort_by_key(|request| (request.target.id(), request.source.map(Entity::id)));
        for request in self.requests.drain(..) {
            resolve_effect(world, &request);
        }
    }
}

/// One payload an instance's period landed, to queue for the step's
/// damage resolve.
enum Payload {
    Damage(DamageRequest),
    Heal(HealRequest),
}

/// What the tick does with an instance after its step.
enum Tick {
    Keep,
    RemoveOne,
    Clear,
}

/// [`EFFECT_TICK`]'s state between steps, kept so a step allocates nothing
/// once warm.
#[derive(Default)]
struct Ticker {
    entities: Vec<Entity>,
    payloads: Vec<Payload>,
    expired: Vec<EffectInstance>,
    /// The instances held last step and this step, as (entity, serial)
    /// sorted by index and serial: the trace notes each suppression once.
    suppressed: Vec<(Entity, u64)>,
    suppressed_now: Vec<(Entity, u64)>,
}

impl Ticker {
    /// Per entity in index order: the pending entries queued, then each
    /// instance's period and time left counted.
    fn run(&mut self, world: &mut World) {
        let dt = world
            .get_resource::<Time>()
            .map_or(0.0, Time::delta)
            .max(0.0);
        self.entities.clear();
        self.entities.extend(
            world
                .query::<(Entity, &Effects)>()
                .map(|(entity, _)| entity),
        );
        self.entities.sort_by_key(|entity| entity.id());
        let entities = std::mem::take(&mut self.entities);
        for &entity in &entities {
            self.tick_entity(world, entity, dt);
        }
        self.entities = entities;
        if dt > 0.0 {
            self.suppressed_now
                .sort_by_key(|&(entity, serial)| (entity.id(), serial));
            std::mem::swap(&mut self.suppressed, &mut self.suppressed_now);
            self.suppressed_now.clear();
        }
    }

    /// Notes `entity`'s instance `serial` as held this step, tracing it
    /// when it was not held the step before.
    fn note_suppressed(&mut self, world: &mut World, entity: Entity, serial: u64) {
        let held = self
            .suppressed
            .binary_search_by_key(&(entity.id(), serial), |&(e, s)| (e.id(), s))
            .is_ok_and(|at| self.suppressed[at].0 == entity);
        if !held {
            skip(world, entity, "tick", SUPPRESSED);
        }
        self.suppressed_now.push((entity, serial));
    }

    fn tick_entity(&mut self, world: &mut World, entity: Entity, dt: f32) {
        let Some(slot) = world.get_mut::<Effects>(entity) else {
            return;
        };
        let mut effects = std::mem::take(slot);
        for effect in effects.pending.drain(..) {
            queue_request(world, EffectRequest::new(entity, effect), false);
        }
        if dt > 0.0 {
            let mut i = 0;
            while i < effects.instances.len() {
                if suppressed(world, entity, &effects.instances[i]) {
                    let serial = effects.instances[i].serial;
                    self.note_suppressed(world, entity, serial);
                    i += 1;
                    continue;
                }
                let (step, landed) = tick_instance(
                    world,
                    entity,
                    &mut effects.instances[i],
                    dt,
                    &mut self.payloads,
                );
                if landed > 0 {
                    restore_instance(world, entity, &effects.instances[i], landed);
                }
                match step {
                    Tick::Keep => i += 1,
                    Tick::RemoveOne => {
                        let effect = definition(world, &effects.instances[i].effect).cloned();
                        let instance = &mut effects.instances[i];
                        instance.stacks -= 1;
                        if let Some(effect) = effect {
                            instance.left = effect.scaled_duration(instance.factor);
                            remove_modifiers(world, entity, instance.serial);
                            place_modifiers(world, entity, &effect, instance, false);
                        }
                        i += 1;
                    }
                    Tick::Clear => {
                        let instance = effects.instances.remove(i);
                        release(world, entity, &mut effects, &instance);
                        self.expired.push(instance);
                    }
                }
            }
        }
        put_back(world, entity, effects);
        for payload in self.payloads.drain(..) {
            match payload {
                Payload::Damage(request) => {
                    request.queue(world);
                }
                Payload::Heal(request) => {
                    request.queue(world);
                }
            }
        }
        for instance in self.expired.drain(..) {
            send_expired(
                world,
                entity,
                instance.effect.id().map(str::to_owned),
                false,
            );
            let on_expire =
                definition(world, &instance.effect).and_then(|effect| effect.on_expire.clone());
            if let Some(on_expire) = on_expire {
                let mut next = EffectRequest::new(entity, on_expire);
                if let Some(source) = instance.source {
                    next = next.with_source(source);
                }
                queue_request(world, next, true);
            }
        }
    }
}

/// One instance's step: its period counted down by the step's time inside
/// its lifetime, each elapse a payload into `payloads`, at most 16; then
/// its time left, and what its end does, with the elapses, which a restore
/// lands at once.
fn tick_instance(
    world: &World,
    entity: Entity,
    instance: &mut EffectInstance,
    dt: f32,
    payloads: &mut Vec<Payload>,
) -> (Tick, u32) {
    let EffectInstance {
        effect: effect_ref,
        source,
        stacks,
        left,
        period_left,
        factor,
        ..
    } = instance;
    let effect = definition(world, effect_ref);
    let elapsed = left.map_or(dt, |left| dt.min(left));
    let mut landed = 0;
    match (effect.and_then(|effect| effect.period), *period_left) {
        (Some(period), Some(period_was)) => {
            let effect = effect.unwrap_or_else(|| unreachable!("the period came from it"));
            let mut next = period_was - elapsed;
            let mut elapses = 0;
            while next <= TIME_EPSILON && elapses < MAX_ELAPSES_PER_STEP {
                elapses += 1;
                next += period;
            }
            if next <= TIME_EPSILON {
                // Capped: the rest of the step's time is dropped and the next
                // cycle starts whole, as a cooldown's does.
                next = period;
            }
            *period_left = Some(next);
            landed = elapses;
            let payload_factor = effect.payload_factor(*factor);
            for _ in 0..elapses {
                if let Some(request) = effect.damage_request(entity, *source, payload_factor) {
                    payloads.push(Payload::Damage(request));
                }
                if let Some(request) = effect.heal_request(entity, *source, payload_factor) {
                    payloads.push(Payload::Heal(request));
                }
            }
        }
        // A reload gave the effect a period: it counts from now.
        (Some(period), None) => *period_left = Some(period),
        (None, Some(_)) => *period_left = None,
        (None, None) => {}
    }
    let Some(was) = *left else {
        return (Tick::Keep, landed);
    };
    let rest = count_down(was, dt);
    *left = Some(rest);
    if rest > 0.0 {
        return (Tick::Keep, landed);
    }
    let end = match effect.map_or(Expire::Clear, |effect| effect.expire) {
        Expire::RemoveOne if *stacks > 1 => Tick::RemoveOne,
        _ => Tick::Clear,
    };
    (end, landed)
}

/// Registers the `effects` data kind and scene component, the `curves`
/// kind, `EventQueue<EffectApplied>` and `EventQueue<EffectExpired>`, the
/// request queue, [`EFFECT_TICK`] in the tick phase after
/// [`STATS_APPLY`](crate::STATS_APPLY) when it is registered,
/// [`EFFECT_RESOLVE`] in the resolve phase after
/// [`DAMAGE_RESOLVE`](crate::DAMAGE_RESOLVE) and the ailment feed when
/// they are registered, and the ailment ladder's `ailment_threshold`
/// component, [`AILMENT_FEED`](crate::AILMENT_FEED) and
/// [`AILMENT_TICK`](crate::AILMENT_TICK), since the ladder applies
/// effects.
///
/// A modifier placed in one step's resolve reaches the movers, health and
/// cooldowns through the next step's `stats_apply`, and an expiry comes
/// after its step's, so an effect of `k` steps moves its consumer for `k`
/// steps. Each step, [`EFFECT_TICK`] queues a fresh component's pending
/// list, counts each instance's period down by the step's time inside its
/// lifetime (so no payload falls past the end), each elapse queueing a
/// periodic [`DamageRequest`](crate::DamageRequest) or
/// [`HealRequest`](crate::HealRequest) for this step's damage resolve, at
/// most 16 a step, a restore's raising its stat's base at once, and counts
/// its time left, unless the entity carries a `suppressed_by` tag, which
/// holds the instance whole for the step: at 0, `clear` removes the
/// instance with its modifiers and tags, sends [`EffectExpired`] and queues
/// its `on_expire`; `remove_one` drops a stack and restarts the time, the
/// last stack ending as `clear`. [`EFFECT_RESOLVE`] applies the step's
/// `on_expire` transitions first, in the order the tick queued them, then
/// the other requests sorted by target and source, so a transition's tag
/// precedes the step's new applications; every kit path queues, and
/// [`EffectRequest::apply`] is the game's immediate form.
///
/// # Examples
///
/// ```
/// use tungsten_core::{Plugin, Schedule, Stage, World};
/// use tungsten_kit::{EFFECT_RESOLVE, EFFECT_TICK, EffectsPlugin};
///
/// let mut world = World::new();
/// let mut schedule = Schedule::new();
/// EffectsPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
/// assert_eq!(schedule.stage_of(EFFECT_TICK), Some(Stage::FixedUpdate));
/// assert_eq!(schedule.stage_of(EFFECT_RESOLVE), Some(Stage::FixedUpdate));
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct EffectsPlugin;

impl Plugin for EffectsPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows_with(world, "Effects", |world, entity| {
            world
                .get::<Effects>(entity)
                .map_or_else(Vec::new, |effects| effects.rows(Some((world, entity))))
        });
        register_curves(world);
        describe_effects(world);
        world.register_event::<EffectApplied>();
        world.register_event::<EffectExpired>();
        if !world.has_resource::<EffectRequests>() {
            world.insert_resource(EffectRequests::default());
        }
        world.register_data_kind_with_assets(EFFECTS, |effect: &Effect, world: &World| {
            effect.check(world)
        });
        world.register_component_with_assets(
            EFFECTS,
            |def: &EffectsDef, world: &World| def.check(world),
            |def: EffectsDef, _: &SpawnContext<'_>| {
                let mut effects = Effects::new();
                effects.pending = def.0;
                (effects,)
            },
        );
        let mut ticker = Ticker::default();
        schedule.add(
            Phase::Tick.stage(),
            Phase::Tick
                .join(system(EFFECT_TICK, move |world: &mut World| {
                    ticker.run(world);
                }))
                .after_if_present(STATS_APPLY),
        );
        let mut resolver = EffectResolver::default();
        schedule.add(
            Phase::Resolve.stage(),
            Phase::Resolve
                .join(system(EFFECT_RESOLVE, move |world: &mut World| {
                    resolver.run(world);
                }))
                .after_if_present(DAMAGE_RESOLVE)
                .after_if_present(AILMENT_FEED),
        );
        register_ailments(schedule, world);
    }
}

/// The descriptions of `effects`, its shapes and the `effects` kind.
fn describe_effects(world: &mut World) {
    describe(
        world,
        ComponentDescription::new(
            EFFECTS,
            "The effects the entity carries: entries applied at its first step, and the active instances.",
        )
        .field(FieldDescription::new(
            "list",
            FieldType::List(Box::new(FieldType::Data(EFFECTS))),
            "the value itself: each an `effects` entry's ID or an effect in place, applied with no source; the load checks each",
        ))
        .event("EffectApplied")
        .event("EffectExpired")
        .genre(PLATFORMER)
        .genre(GRID)
        .sample(r#"["regen"]"#)
        .roundtrip(round_trip::<EffectsDef>),
    );
    let open = FieldType::F32 {
        min: Bound::Open,
        max: Bound::Open,
    };
    let shapes = [
        ComponentDescription::new(
            EFFECT,
            "A change an entity carries: timed, infinite or instant, with a payload, modifiers and tags.",
        )
        .field(
            FieldDescription::new(
                "duration",
                FieldType::F32 {
                    min: Bound::Inclusive(0.0),
                    max: Bound::Open,
                },
                "seconds of game time it lasts; infinite when left out, instant at 0",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "period",
                FieldType::F32 {
                    min: Bound::Exclusive(0.0),
                    max: Bound::Open,
                },
                "seconds between its payloads; once at the application when left out",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "damage",
                FieldType::Damage,
                "the payload's hit, periodic: it passes the window and starts none; a map's keys are declared damage types",
            )
            .default("0.0"),
        )
        .field(
            FieldDescription::new(
                "heal",
                FieldType::F32 {
                    min: Bound::Inclusive(0.0),
                    max: Bound::Open,
                },
                "the payload's heal",
            )
            .default("0.0"),
        )
        .field(
            FieldDescription::new(
                "stack",
                FieldType::Shape(EFFECT_STACK),
                "how many stacks, what a repeat does to the time left, and what an end does",
            )
            .default(r#"{ "limit": 1, "refresh": "refresh", "expire": "clear" }"#),
        )
        .field(
            FieldDescription::new(
                "modifiers",
                FieldType::List(Box::new(FieldType::Shape(STAT_MODIFIER))),
                "on the target's stats, per stack, under the source `effect#<serial>`",
            )
            .default("[]"),
        )
        .field(
            FieldDescription::new(
                "resist",
                FieldType::List(Box::new(FieldType::Shape(RESIST_MODIFIER))),
                "on the target's resistances, per stack",
            )
            .default("[]"),
        )
        .field(
            FieldDescription::new(
                "grant",
                FieldType::List(Box::new(FieldType::Tag)),
                "tags the target carries while an instance runs; the load checks each",
            )
            .default("[]"),
        )
        .field(
            FieldDescription::new(
                "blocked_by",
                FieldType::List(Box::new(FieldType::Tag)),
                "tags that refuse the effect; the load checks each",
            )
            .default("[]"),
        )
        .field(
            FieldDescription::new(
                "restore",
                FieldType::Shape(EFFECT_RESTORE),
                "raises one of the target's stat bases with each payload, at once in the tick, up to another stat's value",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "suppressed_by",
                FieldType::List(Box::new(FieldType::Tag)),
                "tags that hold a running instance: its time and period wait, no payload lands; the load checks each",
            )
            .default("[]"),
        )
        .field(
            FieldDescription::new(
                "scale",
                FieldType::Shape(EFFECT_SCALE),
                "a curve at the request's scale, multiplying what `into` names; the load refuses a `y` below 0",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "on_expire",
                FieldType::Data(EFFECTS),
                "applied to the target from the same source at a timed end; the load checks it",
            )
            .default("null"),
        )
        .sample("{}")
        .roundtrip(round_trip::<EffectDef>),
        ComponentDescription::new(EFFECT_STACK, "An effect's stacking.")
            .field(
                FieldDescription::new(
                    "limit",
                    FieldType::U32 {
                        min: Some(1),
                        max: None,
                    },
                    "the most stacks an instance holds",
                )
                .default("1"),
            )
            .field(
                FieldDescription::new(
                    "refresh",
                    FieldType::Enum(&["refresh", "keep", "extend"]),
                    "what a request on an entity that carries it does to the time left",
                )
                .default(r#""refresh""#),
            )
            .field(
                FieldDescription::new(
                    "expire",
                    FieldType::Enum(&["clear", "remove_one"]),
                    "what a timed end does: every stack, or one at a time",
                )
                .default(r#""clear""#),
            )
            .sample("{}")
            .roundtrip(round_trip::<StackDef>),
        ComponentDescription::new(
            EFFECT_RESTORE,
            "A payload that raises one of the target's stat bases, capped by another stat.",
        )
        .field(FieldDescription::new(
            "stat",
            FieldType::Text,
            "the stat whose base it raises, `[a-z][a-z0-9_]*`; a target without it takes nothing",
        ))
        .field(FieldDescription::new(
            "amount",
            FieldType::F32 {
                min: Bound::Exclusive(0.0),
                max: Bound::Open,
            },
            "added to the base with each payload, times the payload's factor",
        ))
        .field(FieldDescription::new(
            "max",
            FieldType::Text,
            "the stat whose value caps the base; a base already there is left alone",
        ))
        .sample(r#"{ "stat": "mana", "amount": 1.5, "max": "max_mana" }"#)
        .roundtrip(round_trip::<RestoreDef>),
        ComponentDescription::new(STAT_MODIFIER, "A modifier on one of the target's stats.")
            .field(FieldDescription::new(
                "stat",
                FieldType::Text,
                "the stat, `[a-z][a-z0-9_]*`; a target without it takes nothing",
            ))
            .field(FieldDescription::new(
                "modifier",
                FieldType::Shape(MODIFIER),
                "the change; `add` and `pct` scale with the effect's factor",
            ))
            .sample(r#"{ "stat": "move_speed", "modifier": { "pct": -0.2 } }"#)
            .roundtrip(round_trip::<StatModifierDef>),
        ComponentDescription::new(
            RESIST_MODIFIER,
            "A modifier on the target's resistance to one damage tag.",
        )
        .field(FieldDescription::new(
            "tag",
            FieldType::Tag,
            "the damage tag; the load checks it",
        ))
        .field(FieldDescription::new(
            "modifier",
            FieldType::Shape(MODIFIER),
            "the change, over a base of 1 for a tag the target lists no factor for",
        ))
        .sample(r#"{ "tag": "arcane", "modifier": { "mul": 2 } }"#)
        .roundtrip(round_trip::<ResistModifierDef>),
        ComponentDescription::new(
            MODIFIER,
            "A change to a value: added to the base, added to the percentage factor, multiplied, or replaced.",
        )
        .one_of()
        .field(FieldDescription::new("add", open.clone(), "added to the base"))
        .field(FieldDescription::new(
            "pct",
            open.clone(),
            "added to the percentage factor, which starts at 1",
        ))
        .field(FieldDescription::new("mul", open.clone(), "multiplies the result"))
        .field(FieldDescription::new(
            "override",
            open,
            "replaces the result; the last-added override wins",
        ))
        .sample(r#"{ "add": 1 }"#)
        .roundtrip(round_trip::<Modifier>),
        ComponentDescription::new(
            EFFECT_SCALE,
            "A curve at the request's scale, as a factor on what `into` names.",
        )
        .field(FieldDescription::new(
            "curve",
            FieldType::Data(CURVES),
            "every `y` at least 0; the load checks it",
        ))
        .field(
            FieldDescription::new(
                "into",
                FieldType::List(Box::new(FieldType::Enum(INTO_NAMES))),
                "what the factor multiplies: the duration, the damage and heal payload, the `add` and `pct` modifiers",
            )
            .default(r#"["duration", "periodic", "modifiers"]"#),
        )
        .sample(r#"{ "curve": "burn_scale" }"#)
        .roundtrip(round_trip::<ScaleDef>),
    ];
    for shape in shapes {
        describe_shape(world, shape);
    }
    describe_kind(
        world,
        EFFECTS,
        "An `effect`: what a cue's `effect` action, an ailment, an ability or an offer applies.",
    );
}

impl Inspectable for Effects {
    /// Each instance in a line, the pending entries and the granted tags.
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        self.rows(None)
    }
}

impl Effects {
    /// [`inspect_rows`](Inspectable::inspect_rows); with the World and the
    /// entity, an instance's line also names its restore and whether a tag
    /// holds it.
    fn rows(&self, at: Option<(&World, Entity)>) -> Vec<(&'static str, String)> {
        let name = |effect: &DataRef<Effect>| effect.id().unwrap_or("inline").to_owned();
        let mut rows: Vec<(&'static str, String)> = self
            .instances
            .iter()
            .map(|instance| {
                let mut line = format!("{} ×{}", name(&instance.effect), instance.stacks);
                match instance.left {
                    Some(left) => {
                        let _ = write!(line, ", {left:.2} s");
                    }
                    None => line.push_str(", infinite"),
                }
                if let Some(period_left) = instance.period_left {
                    let _ = write!(line, ", next payload {period_left:.2} s");
                }
                if instance.factor != 1.0 {
                    let _ = write!(line, ", ×{}", instance.factor);
                }
                if let Some(source) = instance.source {
                    let _ = write!(line, ", from {source}");
                }
                if let Some((world, entity)) = at {
                    if let Some((stat, ..)) =
                        definition(world, &instance.effect).and_then(Effect::restore)
                    {
                        let _ = write!(line, ", restores {stat}");
                    }
                    if suppressed(world, entity, instance) {
                        line.push_str(", suppressed");
                    }
                }
                ("effect", line)
            })
            .collect();
        if !self.pending.is_empty() {
            let pending: Vec<String> = self.pending.iter().map(name).collect();
            rows.push(("pending", pending.join(", ")));
        }
        if !self.granted.is_empty() {
            let granted: Vec<String> = self
                .granted
                .iter()
                .map(|row| format!("{} ({})", row.tag, row.owners))
                .collect();
            rows.push(("granted", granted.join(", ")));
        }
        rows
    }
}

#[cfg(test)]
#[path = "../tests/effects.rs"]
mod tests;
