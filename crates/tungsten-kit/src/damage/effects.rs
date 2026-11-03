//! Effects: timed, infinite or instant changes an entity carries, with a
//! periodic payload, stacking, modifiers on its stats and resistances,
//! granted, blocking and suppressing tags, and a transition at the end; the
//! `effects` data kind, the `effects` scene component and the requests that
//! apply them. An effect with an `intensity` is a status (redo 7.10b): an
//! application builds its intensity, which decays, reaches bands and scales
//! its curve; a `fuel` burns one of its holder's stats.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use tungsten_core::save::{RefCodec, RefReader, RefWriter, SaveOptions};
use tungsten_core::{
    DataRef, DataRegistry, Entity, EventQueue, Inspectable, MissingAsset, Plugin, Schedule,
    SpawnContext, Time, World, system,
};

use crate::chemistry::outputs::check_outputs;
use crate::chemistry::queue::StatusList;
use crate::chemistry::rules::check_stimulus;
use crate::chemistry::stimuli::{Named, emit_from};
use crate::chemistry::{CHEMISTRY, Chemistry, Output, queue_status_list};
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
use crate::foundation::lines::Line;
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::saved::order_of;
use crate::foundation::stats::{Modifier, STATS_APPLY, Stats};
use crate::foundation::tags::{TagFilter, Tags, check_declared, valid_tag};
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
const EFFECT_INTENSITY: &str = "effect_intensity";
const EFFECT_BAND: &str = "effect_band";
const EFFECT_FUEL: &str = "effect_fuel";
const EFFECT_EMIT: &str = "effect_emit";
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
/// An `intensity` makes it a status, the ARPG design's §5.2 model: an
/// instance holds an intensity in `(0, max]`, which an application builds
/// by its request's amount times its status effect
/// ([`EffectRequest::with_amount`]), capped at `max`, refreshing its time
/// by `stack.refresh` and keeping one stack; each step takes `decay` a
/// second off, inside its lifetime, and at 0 it has run out, an expiry. Its
/// `bands` grant their tags while the intensity is at least their `at`, by
/// the same ownership as `grant`; its `scale` curve reads the intensity, not
/// the request's scale, so its modifiers follow it from the step it changes
/// and its payload at each tick. A `fuel` drains one of its holder's stat
/// bases by `rate` a second while an instance runs: at 0, or on a holder
/// without that stat, it has run out.
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
///   "on_expire": "thawed",
///   "intensity": { "max": 4, "decay": 0.5 },
///   "bands": [{ "at": 1, "grant": ["status.smoking"] }, { "at": 3, "grant": ["status.ablaze"] }],
///   "fuel": { "stat": "oil", "rate": 1 }
/// }
/// ```
///
/// `damage` a number or declared damage types to amounts; a `restore` names
/// its three fields, its amount above 0; `bands` need an `intensity`, their
/// `at` strictly ascending in `(0, max]`; a `fuel`'s rate is above 0; the
/// load checks the damage types, the tags, the curve (every `y` at least 0)
/// and the `on_expire` effect.
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
    modifiers: Vec<(String, Line)>,
    resist: Vec<ResistChange>,
    /// Sorted and deduplicated.
    grant: Vec<String>,
    blocked_by: Vec<String>,
    restore: Option<Restore>,
    /// Sorted and deduplicated.
    suppressed_by: Vec<String>,
    scale: Option<(DataRef<Curve>, Scaled)>,
    on_expire: Option<DataRef<Effect>>,
    intensity: Option<Intensity>,
    /// Strictly ascending by `at`, inside `(0, max]`.
    bands: Vec<Band>,
    fuel: Option<Fuel>,
    /// Run on the holder when an instance ends.
    on_end: Vec<Output>,
    emit: Option<Emit>,
}

/// A status's intensity: the most an instance holds, and what a second
/// takes off.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Intensity {
    max: f32,
    decay: f32,
}

/// One band of a status: the tags it grants while the intensity is at least
/// `at`.
#[derive(Debug, Clone, PartialEq)]
struct Band {
    at: f32,
    /// Sorted and deduplicated.
    grant: Vec<String>,
    /// Run on the holder each time the intensity reaches it from below.
    enter: Vec<Output>,
}

/// A fuel: the holder's stat an instance burns, and how much a second.
#[derive(Debug, Clone, PartialEq)]
struct Fuel {
    stat: String,
    rate: f32,
}

/// An emission: every `period` of an instance's life its holder sends
/// `stimulus` of `amount` (times the instance's factor) to every chemistry
/// receiver within `radius` of it but itself.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Emit {
    pub(crate) stimulus: String,
    pub(crate) amount: f32,
    pub(crate) radius: f32,
    pub(crate) period: f32,
}

/// One change an effect makes to the target's resistance to a tag: a line
/// on its factor, scoped or not, a modifier on its floor, or both.
#[derive(Debug, Clone, PartialEq)]
struct ResistChange {
    tag: String,
    line: Option<Line>,
    floor: Option<Modifier>,
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
            intensity: None,
            bands: Vec::new(),
            fuel: None,
            on_end: Vec::new(),
            emit: None,
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
        self.modifiers.push((stat.to_owned(), Line::new(modifier)));
        self
    }

    /// The effect, putting `line` on the target's stat `stat` per stack,
    /// after its earlier modifiers: an unscoped line as
    /// [`with_modifier`](Self::with_modifier) puts it, a scoped one kept
    /// for the target's reads in context
    /// ([`Stats::add_line`](crate::Stats::add_line)).
    ///
    /// # Panics
    ///
    /// When `stat` is not `[a-z][a-z0-9_]*`.
    #[must_use]
    #[track_caller]
    pub fn with_line(mut self, stat: &str, line: Line) -> Self {
        assert!(
            valid_stat(stat),
            "Effect::with_line: `{stat}` is not a stat name (`[a-z][a-z0-9_]*`)"
        );
        self.modifiers.push((stat.to_owned(), line));
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
        self.resist.push(ResistChange {
            tag: tag.to_owned(),
            line: Some(Line::new(modifier)),
            floor: None,
        });
        self
    }

    /// The effect, putting `line` on the target's resistance to `tag` per
    /// stack, after its earlier ones: an unscoped line as
    /// [`with_resist`](Self::with_resist) puts it, a scoped one read only in
    /// a hit whose context passes it ([`Resist::add_line`]).
    ///
    /// # Panics
    ///
    /// When `tag` is not a tag name.
    #[must_use]
    #[track_caller]
    pub fn with_resist_line(mut self, tag: &str, line: Line) -> Self {
        assert!(
            valid_tag(tag),
            "Effect::with_resist_line: `{tag}` is not a tag name (`[a-z][a-z0-9_]*` segments joined by dots)"
        );
        self.resist.push(ResistChange {
            tag: tag.to_owned(),
            line: Some(line),
            floor: None,
        });
        self
    }

    /// The effect, putting `modifier` on the floor of the target's
    /// resistance to `tag` per stack ([`Resist::add_floor_modifier`]).
    ///
    /// # Panics
    ///
    /// When `tag` is not a tag name or the modifier's value is not finite.
    #[must_use]
    #[track_caller]
    pub fn with_resist_floor(mut self, tag: &str, modifier: Modifier) -> Self {
        assert!(
            valid_tag(tag),
            "Effect::with_resist_floor: `{tag}` is not a tag name (`[a-z][a-z0-9_]*` segments joined by dots)"
        );
        assert!(
            modifier.is_finite(),
            "Effect::with_resist_floor: {tag}: the modifier must be finite (got {modifier:?})"
        );
        self.resist.push(ResistChange {
            tag: tag.to_owned(),
            line: None,
            floor: Some(modifier),
        });
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

    /// The effect as a status: an intensity of at most `max`, which an
    /// application builds and which `decay` a second wears down, the
    /// instance running out at 0.
    ///
    /// # Panics
    ///
    /// When `max` is not finite and above 0, `decay` is not finite and at
    /// least 0, or a band already added lies above `max`.
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
    /// let crate_ = world.spawn();
    /// let burning = DataRef::inline(Effect::new().with_intensity(4.0, 0.5));
    /// for _ in 0..3 {
    ///     EffectRequest::new(crate_, burning.clone()).with_amount(1.5).apply(&mut world);
    /// }
    /// let instance = world.get::<Effects>(crate_).unwrap().instances().next().unwrap();
    /// assert_eq!((instance.intensity(), instance.stacks()), (Some(4.0), 1), "capped, one stack");
    /// ```
    #[must_use]
    #[track_caller]
    pub fn with_intensity(mut self, max: f32, decay: f32) -> Self {
        assert!(
            max.is_finite() && max > 0.0,
            "Effect::with_intensity: max must be finite and > 0 (got {max})"
        );
        assert!(
            decay.is_finite() && decay >= 0.0,
            "Effect::with_intensity: decay must be finite and >= 0 (got {decay})"
        );
        if let Some(band) = self.bands.last() {
            assert!(
                band.at <= max,
                "Effect::with_intensity: a band at {} lies above max {max}",
                band.at
            );
        }
        self.intensity = Some(Intensity { max, decay });
        self
    }

    /// The effect, granting `tags` while its intensity is at least `at`.
    ///
    /// # Panics
    ///
    /// When the effect has no intensity yet, `at` is not finite, above 0,
    /// at most its max and above the last band's, or a name is not a tag
    /// name.
    #[must_use]
    #[track_caller]
    pub fn with_band(mut self, at: f32, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let Some(intensity) = self.intensity else {
            panic!("Effect::with_band: the effect has no intensity; call with_intensity first");
        };
        assert!(
            at.is_finite() && at > 0.0 && at <= intensity.max,
            "Effect::with_band: at must be finite and in (0, {}] (got {at})",
            intensity.max
        );
        if let Some(last) = self.bands.last() {
            assert!(
                at > last.at,
                "Effect::with_band: at must be above the last band's {} (got {at})",
                last.at
            );
        }
        self.bands.push(Band {
            at,
            grant: tag_list("Effect::with_band", tags),
            enter: Vec::new(),
        });
        self
    }

    /// The effect, burning its holder's stat `stat` by `rate` a second
    /// while an instance runs; at 0, or on a holder without the stat, it
    /// has run out.
    ///
    /// # Panics
    ///
    /// When `stat` is not `[a-z][a-z0-9_]*` or `rate` is not finite and
    /// above 0.
    #[must_use]
    #[track_caller]
    pub fn with_fuel(mut self, stat: &str, rate: f32) -> Self {
        assert!(
            valid_stat(stat),
            "Effect::with_fuel: `{stat}` is not a stat name (`[a-z][a-z0-9_]*`)"
        );
        assert!(
            rate.is_finite() && rate > 0.0,
            "Effect::with_fuel: rate must be finite and > 0 (got {rate})"
        );
        self.fuel = Some(Fuel {
            stat: stat.to_owned(),
            rate,
        });
        self
    }

    /// The effect, its holder emitting the chemistry stimulus `stimulus` of
    /// `amount` (times the instance's factor) to every receiver within
    /// `radius` of it but itself, every `period` of an instance's life, on
    /// a clock counted as the payload's period is and saved with it,
    /// credited to the instance's source.
    ///
    /// # Panics
    ///
    /// When `stimulus` is not a tag name or `amount`, `radius` or `period`
    /// is not finite and above 0.
    ///
    /// # Examples
    ///
    /// ```
    /// use tungsten_kit::Effect;
    ///
    /// let burning = Effect::new().with_intensity(4.0, 0.5).with_emit("heat", 1.0, 50.0, 0.25);
    /// let data = serde_json::to_value(&burning).unwrap();
    /// assert_eq!(data["emit"]["radius"], 50.0);
    /// ```
    #[must_use]
    #[track_caller]
    pub fn with_emit(mut self, stimulus: &str, amount: f32, radius: f32, period: f32) -> Self {
        assert!(
            valid_tag(stimulus),
            "Effect::with_emit: `{stimulus}` is not a stimulus name (`[a-z][a-z0-9_]*` segments joined by dots)"
        );
        for (field, value) in [("amount", amount), ("radius", radius), ("period", period)] {
            assert!(
                value.is_finite() && value > 0.0,
                "Effect::with_emit: {field} must be finite and > 0 (got {value})"
            );
        }
        self.emit = Some(Emit {
            stimulus: stimulus.to_owned(),
            amount,
            radius,
            period,
        });
        self
    }

    /// Its emission, when it has one.
    pub(crate) fn emission(&self) -> Option<&Emit> {
        self.emit.as_ref()
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

    /// Its stat modifiers, in order, each line's modifier.
    pub fn modifiers(&self) -> impl Iterator<Item = (&str, Modifier)> {
        self.modifiers
            .iter()
            .map(|(stat, line)| (stat.as_str(), line.modifier()))
    }

    /// Its stat lines with their filters, in order.
    pub fn lines(&self) -> impl Iterator<Item = (&str, &Line)> {
        self.modifiers
            .iter()
            .map(|(stat, line)| (stat.as_str(), line))
    }

    /// Its resistance modifiers, in order, each line's modifier.
    pub fn resist(&self) -> impl Iterator<Item = (&str, Modifier)> {
        self.resist.iter().filter_map(|change| {
            change
                .line
                .as_ref()
                .map(|line| (change.tag.as_str(), line.modifier()))
        })
    }

    /// Its resistance lines with their filters, in order.
    pub fn resist_lines(&self) -> impl Iterator<Item = (&str, &Line)> {
        self.resist
            .iter()
            .filter_map(|change| change.line.as_ref().map(|line| (change.tag.as_str(), line)))
    }

    /// Its modifiers on resistance floors, in order.
    pub fn resist_floors(&self) -> impl Iterator<Item = (&str, Modifier)> {
        self.resist
            .iter()
            .filter_map(|change| change.floor.map(|floor| (change.tag.as_str(), floor)))
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

    /// A status's intensity: its max and its decay a second; `None` for an
    /// effect that is no status.
    #[must_use]
    pub fn intensity(&self) -> Option<(f32, f32)> {
        self.intensity
            .map(|intensity| (intensity.max, intensity.decay))
    }

    /// Its bands, lowest first: each one's `at` and the tags it grants.
    pub fn bands(&self) -> impl Iterator<Item = (f32, &[String])> {
        self.bands
            .iter()
            .map(|band| (band.at, band.grant.as_slice()))
    }

    /// Its fuel, when it has one: the stat it burns and the rate a second.
    #[must_use]
    pub fn fuel(&self) -> Option<(&str, f32)> {
        self.fuel
            .as_ref()
            .map(|fuel| (fuel.stat.as_str(), fuel.rate))
    }

    /// What it runs on its holder when an instance ends: run out, worn
    /// out, its fuel spent, removed or ended; an instant effect's when it
    /// lands, which expires it at once.
    #[must_use]
    pub fn on_end(&self) -> &[Output] {
        &self.on_end
    }

    /// What band `band` runs on its holder each time the intensity reaches
    /// it from below.
    pub(crate) fn band_enter(&self, band: usize) -> Option<&[Output]> {
        self.bands.get(band).map(|band| band.enter.as_slice())
    }

    /// How many of its bands an intensity of `intensity` reaches; none
    /// without one.
    fn bands_reached(&self, intensity: Option<f32>) -> usize {
        intensity.map_or(0, |intensity| {
            self.bands
                .iter()
                .take_while(|band| intensity >= band.at)
                .count()
        })
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

    /// The damage request of one period of a fixed rate, `amount`: shared
    /// over the payload's types in their proportions (an untyped payload's
    /// one part), periodic and typed, so the source's conversion leaves it
    /// as it is; the rate is fixed, so the payload factor (the scale curve
    /// on the periodic payload, the intensity) leaves it too. `None` when
    /// it comes to nothing.
    fn rated_request(
        &self,
        target: Entity,
        source: Option<Entity>,
        amount: f32,
    ) -> Option<DamageRequest> {
        if amount <= 0.0 {
            return None;
        }
        let total: f64 = match &self.damage {
            DamageDef::Typed(parts) => parts.values().map(|&share| f64::from(share)).sum(),
            DamageDef::Number(_) => 0.0,
        };
        let mut request = match &self.damage {
            DamageDef::Typed(parts) if total > 0.0 => DamageRequest::from_parts(
                target,
                parts.iter().map(|(name, &share)| {
                    let part = (f64::from(amount) * f64::from(share) / total) as f32;
                    DamagePart::new(part).with_tags(Tags::new([name.as_str()]))
                }),
            ),
            _ => DamageRequest::new(target, amount),
        };
        request = request.with_periodic(true).typed();
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
        for (i, (_, line)) in self.modifiers.iter().enumerate() {
            line.check(world, &format!("modifiers.{i}"))?;
        }
        for (i, change) in self.resist.iter().enumerate() {
            check_declared(world, &format!("resist.{i}.tag"), &change.tag)?;
            if let Some(line) = &change.line {
                line.check(world, &format!("resist.{i}"))?;
            }
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
        for (i, band) in self.bands.iter().enumerate() {
            for (j, tag) in band.grant.iter().enumerate() {
                check_declared(world, &format!("bands.{i}.grant.{j}"), tag)?;
            }
            check_outputs(world, &format!("bands.{i}.enter"), &band.enter)?;
        }
        check_outputs(world, "on_end", &self.on_end)?;
        if let Some(emit) = &self.emit {
            check_stimulus(world, "emit.stimulus", &emit.stimulus)?;
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
    // A status's fields, left out at their defaults so an effect without
    // them writes today's bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    intensity: Option<IntensityDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    bands: Vec<BandDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fuel: Option<FuelDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    on_end: Vec<Output>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    emit: Option<EmitDef>,
}

/// An emission as data writes it, every field required.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "EmitFields")]
struct EmitDef {
    stimulus: String,
    amount: f32,
    radius: f32,
    period: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EmitFields {
    stimulus: String,
    amount: f32,
    radius: f32,
    period: f32,
}

impl TryFrom<EmitFields> for EmitDef {
    type Error = String;

    fn try_from(fields: EmitFields) -> Result<Self, Self::Error> {
        if !valid_tag(&fields.stimulus) {
            return Err(format!(
                "stimulus: `{}` is not a stimulus name (`[a-z][a-z0-9_]*` segments joined by dots)",
                fields.stimulus
            ));
        }
        for (field, value) in [
            ("amount", fields.amount),
            ("radius", fields.radius),
            ("period", fields.period),
        ] {
            if !(value.is_finite() && value > 0.0) {
                return Err(format!("{field}: must be finite and > 0 (got {value})"));
            }
        }
        Ok(Self {
            stimulus: fields.stimulus,
            amount: fields.amount,
            radius: fields.radius,
            period: fields.period,
        })
    }
}

/// A status's intensity as data writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "IntensityFields")]
struct IntensityDef {
    max: f32,
    decay: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IntensityFields {
    max: f32,
    #[serde(default)]
    decay: f32,
}

impl TryFrom<IntensityFields> for IntensityDef {
    type Error = String;

    fn try_from(fields: IntensityFields) -> Result<Self, Self::Error> {
        if !(fields.max.is_finite() && fields.max > 0.0) {
            return Err(format!("max: must be finite and > 0 (got {})", fields.max));
        }
        if !(fields.decay.is_finite() && fields.decay >= 0.0) {
            return Err(format!(
                "decay: must be finite and >= 0 (got {})",
                fields.decay
            ));
        }
        Ok(Self {
            max: fields.max,
            decay: fields.decay,
        })
    }
}

/// A band as data writes it; its place among the others is the effect's
/// check.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "BandFields")]
struct BandDef {
    at: f32,
    grant: TagList,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    enter: Vec<Output>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BandFields {
    at: f32,
    #[serde(default)]
    grant: TagList,
    #[serde(default)]
    enter: Vec<Output>,
}

impl TryFrom<BandFields> for BandDef {
    type Error = String;

    fn try_from(fields: BandFields) -> Result<Self, Self::Error> {
        if !(fields.at.is_finite() && fields.at > 0.0) {
            return Err(format!("at: must be finite and > 0 (got {})", fields.at));
        }
        Ok(Self {
            at: fields.at,
            grant: fields.grant,
            enter: fields.enter,
        })
    }
}

/// A fuel as data writes it, both fields required.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "FuelFields")]
struct FuelDef {
    stat: String,
    rate: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FuelFields {
    stat: String,
    rate: f32,
}

impl TryFrom<FuelFields> for FuelDef {
    type Error = String;

    fn try_from(fields: FuelFields) -> Result<Self, Self::Error> {
        if !valid_stat(&fields.stat) {
            return Err(format!(
                "stat: `{}` is not a stat name (`[a-z][a-z0-9_]*`)",
                fields.stat
            ));
        }
        if !(fields.rate.is_finite() && fields.rate > 0.0) {
            return Err(format!(
                "rate: must be finite and > 0 (got {})",
                fields.rate
            ));
        }
        Ok(Self {
            stat: fields.stat,
            rate: fields.rate,
        })
    }
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

/// A stat modifier as data writes it: a line with its stat, each filter
/// left out when it passes everything.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "StatModifierFields")]
pub(crate) struct StatModifierDef {
    stat: String,
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatModifierFields {
    stat: String,
    modifier: Modifier,
    #[serde(default)]
    with: TagFilter,
    #[serde(default, rename = "while")]
    holder: TagFilter,
    #[serde(default)]
    against: TagFilter,
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
        let line = Line::from_parts(fields.modifier, fields.with, fields.holder, fields.against)?;
        Ok(Self::of(fields.stat, &line))
    }
}

impl StatModifierDef {
    pub(crate) fn of(stat: String, line: &Line) -> Self {
        Self {
            stat,
            modifier: line.modifier(),
            with: line.context_filter().clone(),
            holder: line.holder_filter().clone(),
            against: line.against_filter().clone(),
        }
    }

    pub(crate) fn into_line(self) -> (String, Line) {
        let line = Line::from_parts(self.modifier, self.with, self.holder, self.against)
            .expect("a stat modifier's line was checked at its load");
        (self.stat, line)
    }

    /// The load's check of its line's tags, at `field`.
    pub(crate) fn check(&self, world: &World, field: &str) -> Result<(), MissingAsset> {
        self.clone().into_line().1.check(world, field)
    }
}

/// A resistance modifier as data writes it: a change to a tag's factor,
/// with its filters, a change to its floor, or both.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "ResistModifierFields")]
struct ResistModifierDef {
    tag: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    modifier: Option<Modifier>,
    #[serde(default, skip_serializing_if = "TagFilter::is_everything")]
    with: TagFilter,
    #[serde(default, skip_serializing_if = "TagFilter::is_everything")]
    against: TagFilter,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    floor: Option<Modifier>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResistModifierFields {
    tag: String,
    #[serde(default)]
    modifier: Option<Modifier>,
    #[serde(default)]
    with: TagFilter,
    #[serde(default)]
    against: TagFilter,
    #[serde(default)]
    floor: Option<Modifier>,
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
        let line = match fields.modifier {
            Some(modifier) => Some(Line::from_parts(
                modifier,
                fields.with,
                TagFilter::new(),
                fields.against,
            )?),
            None if !(fields.with.is_everything() && fields.against.is_everything()) => {
                return Err("`with` and `against` scope a `modifier`, which is missing".into());
            }
            None => None,
        };
        if let Some(floor) = fields.floor
            && !floor.is_finite()
        {
            return Err(format!("floor: must be finite (got {floor:?})"));
        }
        if line.is_none() && fields.floor.is_none() {
            return Err("a resistance modifier names its `modifier`, its `floor` or both".into());
        }
        Ok(Self::of(ResistChange {
            tag: fields.tag,
            line,
            floor: fields.floor,
        }))
    }
}

impl ResistModifierDef {
    fn of(change: ResistChange) -> Self {
        let (modifier, with, against) = match &change.line {
            Some(line) => (
                Some(line.modifier()),
                line.context_filter().clone(),
                line.against_filter().clone(),
            ),
            None => (None, TagFilter::new(), TagFilter::new()),
        };
        Self {
            tag: change.tag,
            modifier,
            with,
            against,
            floor: change.floor,
        }
    }

    fn into_change(self) -> ResistChange {
        let line = self.modifier.map(|modifier| {
            Line::from_parts(modifier, self.with, TagFilter::new(), self.against)
                .expect("a resistance modifier's line was checked at its load")
        });
        ResistChange {
            tag: self.tag,
            line,
            floor: self.floor,
        }
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
        if !def.bands.is_empty() {
            let Some(intensity) = &def.intensity else {
                return Err("bands: a status's bands need its `intensity`".into());
            };
            let mut below = 0.0;
            for (i, band) in def.bands.iter().enumerate() {
                if band.at <= below || band.at > intensity.max {
                    return Err(format!(
                        "bands.{i}.at: must lie above the band before and at most max {} (got {})",
                        intensity.max, band.at
                    ));
                }
                below = band.at;
            }
        }
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
                .map(StatModifierDef::into_line)
                .collect(),
            resist: def
                .resist
                .into_iter()
                .map(ResistModifierDef::into_change)
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
            intensity: def.intensity.map(|intensity| Intensity {
                max: intensity.max,
                decay: intensity.decay,
            }),
            bands: def
                .bands
                .into_iter()
                .map(|band| Band {
                    at: band.at,
                    grant: band.grant.0,
                    enter: band.enter,
                })
                .collect(),
            fuel: def.fuel.map(|fuel| Fuel {
                stat: fuel.stat,
                rate: fuel.rate,
            }),
            on_end: def.on_end,
            emit: def.emit.map(|emit| Emit {
                stimulus: emit.stimulus,
                amount: emit.amount,
                radius: emit.radius,
                period: emit.period,
            }),
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
                .map(|(stat, line)| StatModifierDef::of(stat, &line))
                .collect(),
            resist: effect
                .resist
                .into_iter()
                .map(ResistModifierDef::of)
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
            intensity: effect.intensity.map(|intensity| IntensityDef {
                max: intensity.max,
                decay: intensity.decay,
            }),
            bands: effect
                .bands
                .into_iter()
                .map(|band| BandDef {
                    at: band.at,
                    grant: TagList(band.grant),
                    enter: band.enter,
                })
                .collect(),
            fuel: effect.fuel.map(|fuel| FuelDef {
                stat: fuel.stat,
                rate: fuel.rate,
            }),
            on_end: effect.on_end,
            emit: effect.emit.map(|emit| EmitDef {
                stimulus: emit.stimulus,
                amount: emit.amount,
                radius: emit.radius,
                period: emit.period,
            }),
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
    /// The tags it granted, in the definition's order: `grant`'s, then each
    /// reached band's.
    granted: Vec<String>,
    /// A status's intensity, in `(0, max]`; `None` for an effect that is no
    /// status.
    intensity: Option<f32>,
    /// How many tags each band its intensity reaches granted, lowest
    /// first: their tags close `granted`, in band order.
    bands: Vec<u32>,
    /// Its request's lines, beside the effect's own.
    lines: Vec<(String, Line)>,
    /// Its damage over time a second, fixed at the application from the
    /// hit; its ticks deal it in place of the effect's damage.
    rate: Option<f32>,
    /// Game seconds to its next emission, when the effect emits.
    emit_left: Option<f32>,
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

    /// A status's intensity, in `(0, max]`; `None` for an effect that is
    /// no status.
    #[must_use]
    pub fn intensity(&self) -> Option<f32> {
        self.intensity
    }

    /// The lines its request carried ([`EffectRequest::with_lines`]),
    /// placed beside the effect's own and saved with it.
    pub fn lines(&self) -> impl Iterator<Item = (&str, &Line)> {
        self.lines.iter().map(|(stat, line)| (stat.as_str(), line))
    }

    /// Its damage over time a second, when an application fixed one
    /// ([`EffectRequest::with_rate`]): each period deals `rate × period`,
    /// shared over the effect's damage types in their proportions.
    #[must_use]
    pub fn rate(&self) -> Option<f32> {
        self.rate
    }

    /// Game seconds to its next emission, when the effect emits.
    #[cfg(test)]
    pub(crate) fn emit_left(&self) -> Option<f32> {
        self.emit_left
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
    // A status's state, left out at its defaults so an instance without it
    // writes today's bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    intensity: Option<f32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    bands: Vec<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    lines: Vec<StatModifierDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    rate: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    emit_left: Option<f32>,
}

fn one_f32() -> f32 {
    1.0
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_one(value: &f32) -> bool {
    *value == 1.0
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
            ("emit_left", fields.emit_left),
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
        if let Some(intensity) = fields.intensity
            && !(intensity.is_finite() && intensity > 0.0)
        {
            return Err(format!(
                "intensity: must be finite and > 0 (got {intensity})"
            ));
        }
        if let Some(rate) = fields.rate
            && !(rate.is_finite() && rate >= 0.0)
        {
            return Err(format!("rate: must be finite and >= 0 (got {rate})"));
        }
        let banded: u64 = fields.bands.iter().map(|&count| u64::from(count)).sum();
        if banded > fields.granted.len() as u64 {
            return Err(format!(
                "bands: {banded} band tags, but {} granted",
                fields.granted.len()
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
            intensity: fields.intensity,
            bands: fields.bands,
            lines: fields
                .lines
                .into_iter()
                .map(StatModifierDef::into_line)
                .collect(),
            rate: fields.rate,
            emit_left: fields.emit_left,
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
            intensity: instance.intensity,
            bands: instance.bands,
            lines: instance
                .lines
                .into_iter()
                .map(|(stat, line)| StatModifierDef::of(stat, &line))
                .collect(),
            rate: instance.rate,
            emit_left: instance.emit_left,
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

    /// The intensity of the entry `id`'s instance, when it is a status's;
    /// `None` without one.
    #[must_use]
    pub fn intensity(&self, id: &str) -> Option<f32> {
        self.instances
            .iter()
            .find(|instance| instance.effect.id() == Some(id))
            .and_then(|instance| instance.intensity)
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
    /// The target is gone, despawning or dead, the effect's entry is
    /// missing, or a status's first build is 0.
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
/// The request's status effect is the one it names
/// ([`with_status_effect`](Self::with_status_effect)), else its source's
/// [`Stats::STATUS_EFFECT`] at the resolve, else 1; it multiplies the
/// request's amount and its scale. A status's application builds its
/// intensity by the amount times the status effect, capped at its max, and
/// keeps one stack; its first build of 0 is [`Applied::Ignored`]. The
/// request's lines ([`with_lines`](Self::with_lines)) go on the instance
/// beside the effect's own, under its source, and a later application's
/// replace them.
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
    amount: f32,
    status_effect: Option<f32>,
    lines: Vec<(String, Line)>,
    rate: Option<f32>,
    /// How deep the outputs it sets off queue: a band's `enter`.
    depth: u8,
}

impl EffectRequest {
    /// A request of `effect` on `target`, from no source, at scale 1 and
    /// amount 1, with no lines.
    #[must_use]
    pub fn new(target: Entity, effect: DataRef<Effect>) -> Self {
        Self {
            target,
            effect,
            source: None,
            scale: 1.0,
            amount: 1.0,
            status_effect: None,
            lines: Vec::new(),
            rate: None,
            depth: 0,
        }
    }

    /// The request, fixing its instance's damage over time at `per_second`:
    /// each of its periods deals `per_second × period`, shared over the
    /// effect's damage types in their proportions, typed at the
    /// application, so the source's conversion and the user's later lines
    /// change nothing; a later application's rate replaces it.
    ///
    /// # Panics
    ///
    /// When `per_second` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_rate(mut self, per_second: f32) -> Self {
        assert!(
            per_second.is_finite() && per_second >= 0.0,
            "EffectRequest::with_rate: per_second must be finite and >= 0 (got {per_second})"
        );
        self.rate = Some(per_second);
        self
    }

    /// The request, its band entries' outputs queued `depth` reactions
    /// deep.
    pub(crate) fn at_depth(mut self, depth: u8) -> Self {
        self.depth = depth;
        self
    }

    /// The request, building a status's intensity by `amount` (times its
    /// status effect); an effect that is no status ignores it.
    ///
    /// # Panics
    ///
    /// When `amount` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_amount(mut self, amount: f32) -> Self {
        assert!(
            amount.is_finite() && amount >= 0.0,
            "EffectRequest::with_amount: amount must be finite and >= 0 (got {amount})"
        );
        self.amount = amount;
        self
    }

    /// The request, at the status effect `status_effect` in place of its
    /// source's [`Stats::STATUS_EFFECT`].
    ///
    /// # Panics
    ///
    /// When `status_effect` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_status_effect(mut self, status_effect: f32) -> Self {
        assert!(
            status_effect.is_finite() && status_effect >= 0.0,
            "EffectRequest::with_status_effect: must be finite and >= 0 (got {status_effect})"
        );
        self.status_effect = Some(status_effect);
        self
    }

    /// The request, carrying `lines` onto its instance: placed under the
    /// instance's source beside the effect's own, per stack and scaled as
    /// they are, saved with the instance and replaced by a later
    /// application's.
    ///
    /// # Panics
    ///
    /// When a stat is not `[a-z][a-z0-9_]*`.
    #[must_use]
    #[track_caller]
    pub fn with_lines(
        mut self,
        lines: impl IntoIterator<Item = (impl Into<String>, Line)>,
    ) -> Self {
        for (stat, line) in lines {
            let stat = stat.into();
            assert!(
                valid_stat(&stat),
                "EffectRequest::with_lines: `{stat}` is not a stat name (`[a-z][a-z0-9_]*`)"
            );
            self.lines.push((stat, line));
        }
        self
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

    /// The amount a status's application builds by, before its status
    /// effect.
    #[must_use]
    pub fn amount(&self) -> f32 {
        self.amount
    }

    /// The status effect it names, when it names one.
    #[must_use]
    pub fn status_effect(&self) -> Option<f32> {
        self.status_effect
    }

    /// The lines it carries onto its instance.
    pub fn lines(&self) -> impl Iterator<Item = (&str, &Line)> {
        self.lines.iter().map(|(stat, line)| (stat.as_str(), line))
    }

    /// The damage over time a second it fixes, when it fixes one.
    #[must_use]
    pub fn rate(&self) -> Option<f32> {
        self.rate
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
/// leave, [`EffectExpired`] goes out with `ended` set, its `on_expire`
/// does not apply and its `on_end` outputs queue for chemistry. Whether
/// there was one.
pub fn end_effect(world: &mut World, entity: Entity, effect: &DataRef<Effect>) -> bool {
    end_effect_at(world, entity, effect, 0)
}

/// [`end_effect`], its `on_end` outputs queued `depth` reactions deep: a
/// chemistry `remove` output's.
pub(crate) fn end_effect_at(
    world: &mut World,
    entity: Entity,
    effect: &DataRef<Effect>,
    depth: u8,
) -> bool {
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
    queue_on_end(world, entity, &instance, depth);
    true
}

/// Takes `amount` off the intensity of `effect`'s instance on `entity`, a
/// status's: its bands, factor and modifiers follow, and at 0 it has run
/// out, as a decay does, its `on_end` outputs queued `depth` reactions
/// deep. False, changing nothing, without such a status.
pub(crate) fn consume(
    world: &mut World,
    entity: Entity,
    effect: &DataRef<Effect>,
    amount: f32,
    depth: u8,
) -> bool {
    let Some(slot) = world.get_mut::<Effects>(entity) else {
        return false;
    };
    let Some(at) = slot.find(effect) else {
        return false;
    };
    let Some(was) = slot.instances[at].intensity else {
        return false;
    };
    let mut effects = std::mem::take(slot);
    let rest = count_down(was, amount);
    if rest > 0.0 {
        effects.instances[at].intensity = Some(rest);
        follow_intensity(world, entity, &mut effects, at);
        put_back(world, entity, effects);
        return true;
    }
    let instance = effects.instances.remove(at);
    release(world, entity, &mut effects, &instance);
    put_back(world, entity, effects);
    expire(world, entity, &instance, depth);
    true
}

/// An instance's end in its time: [`EffectExpired`], its `on_expire`
/// queued as the step's transition and its `on_end` outputs, both `depth`
/// reactions deep (0 from a tick).
fn expire(world: &mut World, entity: Entity, instance: &EffectInstance, depth: u8) {
    send_expired(
        world,
        entity,
        instance.effect.id().map(str::to_owned),
        false,
    );
    let on_expire = definition(world, &instance.effect).and_then(|effect| effect.on_expire.clone());
    if let Some(on_expire) = on_expire {
        let mut next = EffectRequest::new(entity, on_expire).at_depth(depth);
        if let Some(source) = instance.source {
            next = next.with_source(source);
        }
        queue_request(world, next, true);
    }
    queue_on_end(world, entity, instance, depth);
}

/// Queues `instance`'s `on_end` outputs on `entity` for chemistry, `depth`
/// reactions deep, credited to its source, when its effect has any.
fn queue_on_end(world: &mut World, entity: Entity, instance: &EffectInstance, depth: u8) {
    let ends = definition(world, &instance.effect).is_some_and(|effect| !effect.on_end.is_empty());
    if ends {
        let list = StatusList {
            effect: instance.effect.clone(),
            band: None,
        };
        queue_status_list(world, entity, list, depth, instance.source);
    }
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

/// The name a save holds [`EffectRequests`] under.
const EFFECT_REQUESTS: &str = "effect_requests";

/// `Effects` saved with each instance's source by save ID, under the
/// instance's serial.
fn effects_saved() -> SaveOptions<Effects> {
    SaveOptions::<Effects>::new().refs(RefCodec {
        save: |effects, out: &mut RefWriter<'_>| {
            for instance in &effects.instances {
                if let Some(source) = instance.source {
                    out.put(&instance.serial.to_string(), source);
                }
            }
        },
        load: |effects, refs: &RefReader<'_>| {
            for instance in &mut effects.instances {
                if let Some(source) = refs.first(&instance.serial.to_string()) {
                    instance.source = Some(source);
                }
            }
        },
        keys: |effects| {
            effects
                .instances
                .iter()
                .map(|instance| instance.serial.to_string())
                .collect()
        },
    })
}

/// An [`EffectRequest`] in a save, its target and source by reference.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EffectRequestSave {
    effect: DataRef<Effect>,
    #[serde(default = "one_f32")]
    scale: f32,
    #[serde(
        default = "one_f32",
        skip_serializing_if = "is_one",
        deserialize_with = "non_negative"
    )]
    amount: f32,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_non_negative"
    )]
    status_effect: Option<f32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    lines: Vec<StatModifierDef>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_non_negative"
    )]
    rate: Option<f32>,
    #[serde(default, skip_serializing_if = "is_zero")]
    depth: u8,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(value: &u8) -> bool {
    *value == 0
}

impl From<&EffectRequest> for EffectRequestSave {
    fn from(request: &EffectRequest) -> Self {
        Self {
            effect: request.effect.clone(),
            scale: request.scale,
            amount: request.amount,
            status_effect: request.status_effect,
            lines: request
                .lines
                .iter()
                .map(|(stat, line)| StatModifierDef::of(stat.clone(), line))
                .collect(),
            rate: request.rate,
            depth: request.depth,
        }
    }
}

impl From<EffectRequestSave> for EffectRequest {
    fn from(saved: EffectRequestSave) -> Self {
        Self {
            target: Entity::PLACEHOLDER,
            effect: saved.effect,
            source: None,
            scale: saved.scale,
            amount: saved.amount,
            status_effect: saved.status_effect,
            lines: saved
                .lines
                .into_iter()
                .map(StatModifierDef::into_line)
                .collect(),
            rate: saved.rate,
            depth: saved.depth,
        }
    }
}

/// [`EffectRequests`] in a save: the step's expiries and the rest.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EffectRequestsSave {
    #[serde(default)]
    expiries: Vec<EffectRequestSave>,
    #[serde(default)]
    requests: Vec<EffectRequestSave>,
}

impl From<&EffectRequests> for EffectRequestsSave {
    fn from(requests: &EffectRequests) -> Self {
        Self {
            expiries: requests.expiries.iter().map(Into::into).collect(),
            requests: requests.requests.iter().map(Into::into).collect(),
        }
    }
}

impl From<EffectRequestsSave> for EffectRequests {
    fn from(saved: EffectRequestsSave) -> Self {
        Self {
            expiries: saved.expiries.into_iter().map(Into::into).collect(),
            requests: saved.requests.into_iter().map(Into::into).collect(),
        }
    }
}

/// The queued effect requests' lists by name, for their references' keys.
fn effect_lists(requests: &EffectRequests) -> [(&'static str, &Vec<EffectRequest>); 2] {
    [
        ("expiry", &requests.expiries),
        ("request", &requests.requests),
    ]
}

/// The queued effect requests, saved so a request queued after a step's
/// resolve resolves once after a load; each target and source by save ID.
fn effect_requests_saved() -> SaveOptions<EffectRequests> {
    SaveOptions::<EffectRequests>::form::<EffectRequestsSave>().refs(RefCodec {
        save: |requests, out: &mut RefWriter<'_>| {
            for (list, queued) in effect_lists(requests) {
                for (index, request) in queued.iter().enumerate() {
                    out.put(&format!("{list}.{index}.target"), request.target);
                    if let Some(source) = request.source {
                        out.put(&format!("{list}.{index}.source"), source);
                    }
                }
            }
        },
        load: |requests, refs: &RefReader<'_>| {
            for (list, queued) in [
                ("expiry", &mut requests.expiries),
                ("request", &mut requests.requests),
            ] {
                for (index, request) in queued.iter_mut().enumerate() {
                    if let Some(target) = refs.first(&format!("{list}.{index}.target")) {
                        request.target = target;
                    }
                    if let Some(source) = refs.first(&format!("{list}.{index}.source")) {
                        request.source = Some(source);
                    }
                }
            }
        },
        keys: |requests| {
            effect_lists(requests)
                .into_iter()
                .flat_map(|(list, queued)| {
                    (0..queued.len()).flat_map(move |index| {
                        [
                            format!("{list}.{index}.target"),
                            format!("{list}.{index}.source"),
                        ]
                    })
                })
                .collect()
        },
    })
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
const NO_BUILD: &str = "the status builds no intensity";
const NO_FUEL: &str = "the holder has no Stats with the fuel's stat";

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

/// Puts `effect`'s modifiers and the instance's lines on `target` under the
/// instance's source, `stacks` copies each, `add` and `pct` values times
/// the factor when the scale names the modifiers; a target without
/// [`Stats`] or [`Resist`] takes none, traced when `fresh`.
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
    if !(effect.modifiers.is_empty() && instance.lines.is_empty()) {
        match world.get_mut::<Stats>(target) {
            Some(stats) => {
                for _ in 0..instance.stacks {
                    for (stat, line) in effect.modifiers.iter().chain(&instance.lines) {
                        let modifier = scale(line.modifier());
                        if line.is_scoped() {
                            stats.add_line(stat, &source, line.clone().with_modifier(modifier));
                        } else {
                            stats.add_modifier(stat, &source, modifier);
                        }
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
                    for change in &effect.resist {
                        if let Some(line) = &change.line {
                            let modifier = scale(line.modifier());
                            if line.is_scoped() {
                                resist.add_line(
                                    &change.tag,
                                    &source,
                                    line.clone().with_modifier(modifier),
                                );
                            } else {
                                resist.add_modifier(&change.tag, &source, modifier);
                            }
                        }
                        if let Some(floor) = change.floor {
                            resist.add_floor_modifier(&change.tag, &source, scale(floor));
                        }
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
/// component's ownership table `granted`.
fn grant(
    world: &mut World,
    target: Entity,
    granted: &mut Vec<Granted>,
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
        match granted.binary_search_by(|row| row.tag.as_str().cmp(tag)) {
            Ok(at) => granted[at].owners += 1,
            Err(at) => {
                let prior = carried.iter().any(|name| name == tag);
                granted.insert(
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

/// Gives back `tags`, one grant each: a tag leaves the entity when no
/// instance grants it any more and the entity did not carry it before.
fn release_tags(world: &mut World, target: Entity, granted: &mut Vec<Granted>, tags: &[String]) {
    for tag in tags {
        let Ok(at) = granted.binary_search_by(|row| row.tag.as_str().cmp(tag)) else {
            continue;
        };
        let row = &mut granted[at];
        row.owners = row.owners.saturating_sub(1);
        if row.owners > 0 {
            continue;
        }
        let prior = row.prior;
        granted.remove(at);
        if !prior && let Some(carried) = world.get_mut::<Tags>(target) {
            carried.remove(tag);
        }
    }
}

/// Takes `instance` off `target`: its modifiers by source, and each tag on
/// its granted list, its bands' included.
fn release(world: &mut World, target: Entity, effects: &mut Effects, instance: &EffectInstance) {
    remove_modifiers(world, target, instance.serial);
    release_tags(world, target, &mut effects.granted, &instance.granted);
}

/// Brings `instance`'s bands to the ones its intensity reaches: a band
/// fallen below gives back its tags, the highest first; a band reached
/// grants its own, the lowest first, and queues its `enter` outputs
/// `depth` reactions deep. Whether any changed.
fn reach_bands(
    world: &mut World,
    target: Entity,
    granted: &mut Vec<Granted>,
    instance: &mut EffectInstance,
    effect: &Effect,
    depth: u8,
) -> bool {
    let reached = effect.bands_reached(instance.intensity);
    let changed = reached != instance.bands.len();
    while instance.bands.len() > reached {
        let count = instance.bands.pop().map_or(0, |count| count as usize);
        let from = instance.granted.len().saturating_sub(count);
        release_tags(world, target, granted, &instance.granted[from..]);
        instance.granted.truncate(from);
    }
    while instance.bands.len() < reached {
        let at = instance.bands.len();
        let band = &effect.bands[at];
        let before = instance.granted.len();
        grant(world, target, granted, instance, &band.grant);
        instance
            .bands
            .push((instance.granted.len() - before) as u32);
        if !band.enter.is_empty() {
            let list = StatusList {
                effect: instance.effect.clone(),
                band: Some(at as u32),
            };
            queue_status_list(world, target, list, depth, instance.source);
        }
    }
    changed
}

/// The status effect `request` resolves at: the one it names, else its
/// source's [`Stats::STATUS_EFFECT`], else 1.
fn status_effect_of(world: &World, request: &EffectRequest) -> f32 {
    request.status_effect.unwrap_or_else(|| {
        request
            .source
            .and_then(|source| world.get::<Stats>(source))
            .and_then(|stats| stats.get(Stats::STATUS_EFFECT))
            .unwrap_or(1.0)
    })
}

/// A status's intensity after a build of `amount` on `prior`, capped at
/// `max`, in `f64`.
fn built(prior: f32, amount: f32, status_effect: f32, max: f32) -> f32 {
    (f64::from(prior) + f64::from(amount) * f64::from(status_effect)).min(f64::from(max)) as f32
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
    let status_effect = status_effect_of(world, request);
    let existing = world
        .get::<Effects>(target)
        .and_then(|effects| effects.find(&request.effect));
    // A status's curve reads its intensity after the build; any other
    // effect's, the request's scale.
    let intensity = effect.intensity.map(|spec| {
        let prior = existing
            .and_then(|at| world.get::<Effects>(target).unwrap().instances[at].intensity)
            .unwrap_or(0.0);
        built(prior, request.amount, status_effect, spec.max)
    });
    if existing.is_none() && intensity.is_some_and(|intensity| intensity <= 0.0) {
        skip(world, target, "apply", NO_BUILD);
        return Applied::Ignored;
    }
    let factor = match intensity {
        Some(intensity) => factor_of(world, &effect, intensity),
        None => factor_of(
            world,
            &effect,
            scaled(request.scale, f64::from(status_effect)),
        ),
    };
    let id = request.effect.id().map(str::to_owned);
    if effect.scaled_duration(factor) == Some(0.0) {
        // An instant effect lands and expires at once: its `on_expire` and
        // its `on_end` outputs at the request's depth, so a cycle through
        // them deepens and ends at chemistry's depth limit.
        send_applied(world, target, request.source, 1, id.clone());
        land_payload(world, &effect, target, request.source, factor);
        send_expired(world, target, id, false);
        if let Some(on_expire) = &effect.on_expire {
            let mut next = EffectRequest::new(target, on_expire.clone()).at_depth(request.depth);
            if let Some(source) = request.source {
                next = next.with_source(source);
            }
            queue_request(world, next, false);
        }
        if !effect.on_end.is_empty() {
            let list = StatusList {
                effect: request.effect.clone(),
                band: None,
            };
            queue_status_list(world, target, list, request.depth, request.source);
        }
        return Applied::Applied;
    }
    if !world.has::<Effects>(target) {
        world.insert(target, Effects::new());
    }
    let mut effects = std::mem::take(world.get_mut::<Effects>(target).unwrap());
    let Effects {
        instances,
        next_serial,
        granted,
        ..
    } = &mut effects;
    let (outcome, stacks) = match existing {
        None => {
            let serial = *next_serial;
            *next_serial += 1;
            let mut instance = EffectInstance {
                effect: request.effect.clone(),
                source: request.source,
                stacks: 1,
                left: effect.scaled_duration(factor),
                period_left: effect.period,
                factor,
                serial,
                granted: Vec::new(),
                intensity,
                bands: Vec::new(),
                lines: request.lines.clone(),
                rate: request.rate,
                emit_left: effect.emit.as_ref().map(|emit| emit.period),
            };
            place_modifiers(world, target, &effect, &instance, true);
            grant(world, target, granted, &mut instance, &effect.grant);
            reach_bands(
                world,
                target,
                granted,
                &mut instance,
                &effect,
                request.depth,
            );
            instances.push(instance);
            (Applied::Applied, 1)
        }
        Some(at) => {
            let instance = &mut instances[at];
            if intensity.is_none() && instance.stacks < effect.limit {
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
            instance.intensity = intensity;
            if request.source.is_some() {
                instance.source = request.source;
            }
            instance.lines.clone_from(&request.lines);
            instance.rate = request.rate;
            remove_modifiers(world, target, instance.serial);
            place_modifiers(world, target, &effect, instance, false);
            reach_bands(world, target, granted, instance, &effect, request.depth);
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
        let order = order_of(world);
        self.requests.sort_by_key(|request| {
            (
                order.key(request.target),
                request.source.map(|source| order.key(source)),
            )
        });
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
    /// An emission from the holder, for chemistry's next run.
    Emit {
        stimulus: Named,
        amount: f32,
        radius: f32,
        credit: Option<Entity>,
    },
}

/// What the tick does with an instance after its step.
enum Tick {
    Keep,
    RemoveOne,
    Clear,
}

/// One instance's step, as [`tick_instance`] counts it.
struct Ticked {
    end: Tick,
    /// The period elapses it landed.
    landed: u32,
    /// What its fuel burns this step, the stat's name copied into the
    /// ticker's buffer.
    burn: Option<f32>,
    /// A status's intensity wore to 0.
    ran_out: bool,
    /// A status's intensity moved.
    worn: bool,
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
    /// The stat a fuel burns, copied out of its definition.
    fuel: String,
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
        let order = order_of(world);
        self.entities.sort_by_key(|&entity| order.key(entity));
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
                let ticked = tick_instance(
                    world,
                    entity,
                    &mut effects.instances[i],
                    dt,
                    &mut self.payloads,
                    &mut self.fuel,
                );
                if ticked.landed > 0 {
                    restore_instance(world, entity, &effects.instances[i], ticked.landed);
                }
                let mut step = ticked.end;
                if let Some(amount) = ticked.burn
                    && burn_fuel(world, entity, &self.fuel, amount)
                {
                    step = Tick::Clear;
                }
                if ticked.ran_out {
                    step = Tick::Clear;
                }
                if ticked.worn && matches!(step, Tick::Keep) {
                    follow_intensity(world, entity, &mut effects, i);
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
                Payload::Emit {
                    stimulus,
                    amount,
                    radius,
                    credit,
                } => emit_from(world, entity, stimulus, amount, radius, credit, 0),
            }
        }
        let expired = std::mem::take(&mut self.expired);
        for instance in &expired {
            expire(world, entity, instance, 0);
        }
        self.expired = expired;
        self.expired.clear();
    }
}

/// Burns `amount` of `entity`'s stat `stat` off its base, never below 0;
/// whether it has run out: the base at 0, or no such stat, traced.
fn burn_fuel(world: &mut World, entity: Entity, stat: &str, amount: f32) -> bool {
    let Some(base) = world
        .get::<Stats>(entity)
        .and_then(|stats| stats.base(stat))
    else {
        skip(world, entity, "fuel", NO_FUEL);
        return true;
    };
    let rest = count_down(base, amount);
    if rest != base
        && let Some(stats) = world.get_mut::<Stats>(entity)
    {
        stats.set_base(stat, rest);
    }
    rest <= 0.0
}

/// A status's instance `i` on `entity` after its intensity moved: its
/// factor follows its curve, its modifiers are placed again when the factor
/// reaches them and changed, and its bands follow the intensity. The
/// definition is cloned only when something is placed or granted.
fn follow_intensity(world: &mut World, entity: Entity, effects: &mut Effects, i: usize) {
    let instance = &effects.instances[i];
    let Some(effect) = definition(world, &instance.effect) else {
        return;
    };
    let factor = instance.intensity.map_or(instance.factor, |intensity| {
        factor_of(world, effect, intensity)
    });
    let placed = factor != instance.factor
        && effect.scales(Scaled::scales_modifiers)
        && !(effect.modifiers.is_empty() && effect.resist.is_empty() && instance.lines.is_empty());
    let banded = effect.bands_reached(instance.intensity) != instance.bands.len();
    if !(placed || banded) {
        effects.instances[i].factor = factor;
        return;
    }
    let effect = effect.clone();
    let Effects {
        instances, granted, ..
    } = effects;
    let instance = &mut instances[i];
    instance.factor = factor;
    if placed {
        remove_modifiers(world, entity, instance.serial);
        place_modifiers(world, entity, &effect, instance, false);
    }
    if banded {
        reach_bands(world, entity, granted, instance, &effect, 0);
    }
}

/// One instance's step: its period counted down by the step's time inside
/// its lifetime, each elapse a payload into `payloads`, at most 16; its
/// emission's clock by the same time, its elapses one emission while the
/// World holds chemistry; then its fuel's burn and its intensity's decay
/// over the same time; then its time left, and what its end does, with the
/// elapses, which a restore lands at once.
fn tick_instance(
    world: &World,
    entity: Entity,
    instance: &mut EffectInstance,
    dt: f32,
    payloads: &mut Vec<Payload>,
    fuel: &mut String,
) -> Ticked {
    let EffectInstance {
        effect: effect_ref,
        source,
        stacks,
        left,
        period_left,
        factor,
        intensity,
        rate,
        emit_left,
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
                let damage = match *rate {
                    Some(rate) => {
                        effect.rated_request(entity, *source, scaled(rate, f64::from(period)))
                    }
                    None => effect.damage_request(entity, *source, payload_factor),
                };
                if let Some(request) = damage {
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
    match (effect.and_then(Effect::emission), *emit_left) {
        (Some(emit), Some(was)) => {
            let mut next = was - elapsed;
            let mut elapses = 0;
            while next <= TIME_EPSILON && elapses < MAX_ELAPSES_PER_STEP {
                elapses += 1;
                next += emit.period;
            }
            if next <= TIME_EPSILON {
                next = emit.period;
            }
            *emit_left = Some(next);
            // The step's elapses as one emission of their sum, which a
            // merge reads as it reads them apart.
            if elapses > 0
                && let Some(chemistry) = world.get_resource::<Chemistry>()
            {
                let stimulus = match chemistry.find(&emit.stimulus) {
                    Some(id) => Named::Known(id),
                    None => Named::New(emit.stimulus.clone()),
                };
                let times = f64::from(elapses) * f64::from(*factor);
                payloads.push(Payload::Emit {
                    stimulus,
                    amount: scaled(emit.amount, times),
                    radius: emit.radius,
                    credit: *source,
                });
            }
        }
        // A reload gave the effect an emission: it counts from now.
        (Some(emit), None) => *emit_left = Some(emit.period),
        (None, Some(_)) => *emit_left = None,
        (None, None) => {}
    }
    let burn = effect.and_then(|effect| effect.fuel.as_ref()).map(|spec| {
        fuel.clear();
        fuel.push_str(&spec.stat);
        spec.rate * elapsed
    });
    let (mut ran_out, mut worn) = (false, false);
    if let (Some(spec), Some(was)) = (effect.and_then(|effect| effect.intensity), *intensity) {
        let rest = count_down(was, spec.decay * elapsed);
        worn = rest != was;
        if rest > 0.0 {
            *intensity = Some(rest);
        } else {
            ran_out = true;
        }
    }
    let ticked = |end| Ticked {
        end,
        landed,
        burn,
        ran_out,
        worn,
    };
    let Some(was) = *left else {
        return ticked(Tick::Keep);
    };
    let rest = count_down(was, dt);
    *left = Some(rest);
    if rest > 0.0 {
        return ticked(Tick::Keep);
    }
    let end = match effect.map_or(Expire::Clear, |effect| effect.expire) {
        Expire::RemoveOne if *stacks > 1 => Tick::RemoveOne,
        _ => Tick::Clear,
    };
    ticked(end)
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
/// [`EffectRequest::apply`] is the game's immediate form. A status's
/// intensity wears by its decay over the same time as its period, and a
/// fuel burns its stat's base; either at 0 ends the instance as `clear`
/// does, and a status's bands, factor and modifiers follow its intensity in
/// the step it moves.
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
        world.register_saved_with(EFFECTS, effects_saved());
        world.register_saved_resource_with(EFFECT_REQUESTS, effect_requests_saved());
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
                .after_if_present(AILMENT_FEED)
                .after_if_present(CHEMISTRY),
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
        .field(
            FieldDescription::new(
                "intensity",
                FieldType::Shape(EFFECT_INTENSITY),
                "makes it a status: an intensity each application builds by its amount times its status effect, up to `max`, one stack, worn by `decay` a second until it runs out at 0; its `scale` curve reads the intensity",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "bands",
                FieldType::List(Box::new(FieldType::Shape(EFFECT_BAND))),
                "a status's thresholds, `at` strictly ascending up to its `max`: each band's tags granted while the intensity is at least its `at`",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "fuel",
                FieldType::Shape(EFFECT_FUEL),
                "a stat of its holder's an instance burns while it runs; at 0, or on a holder without it, it has run out",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "on_end",
                FieldType::List(Box::new(FieldType::Shape("interaction_output"))),
                "chemistry outputs run on the holder when an instance ends: run out, worn out, its fuel spent, removed or ended; an instant one's as it lands",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "emit",
                FieldType::Shape(EFFECT_EMIT),
                "a chemistry stimulus its holder sends to the receivers around it on a period",
            )
            .default("null"),
        )
        .sample("{}")
        .roundtrip(round_trip::<EffectDef>),
        ComponentDescription::new(
            EFFECT_INTENSITY,
            "A status's intensity: how high an application builds it and how fast it wears.",
        )
        .field(FieldDescription::new(
            "max",
            FieldType::F32 {
                min: Bound::Exclusive(0.0),
                max: Bound::Open,
            },
            "the most an instance holds; a build past it is capped",
        ))
        .field(
            FieldDescription::new(
                "decay",
                FieldType::F32 {
                    min: Bound::Inclusive(0.0),
                    max: Bound::Open,
                },
                "taken off a second, over the step's time inside the lifetime",
            )
            .default("0.0"),
        )
        .sample(r#"{ "max": 4 }"#)
        .roundtrip(round_trip::<IntensityDef>),
        ComponentDescription::new(EFFECT_BAND, "A status's band: tags granted while its intensity reaches it.")
            .field(FieldDescription::new(
                "at",
                FieldType::F32 {
                    min: Bound::Exclusive(0.0),
                    max: Bound::Open,
                },
                "the intensity it starts at, above the band before and at most the status's `max`",
            ))
            .field(
                FieldDescription::new(
                    "grant",
                    FieldType::List(Box::new(FieldType::Tag)),
                    "tags the holder carries while the intensity is at least `at`; the load checks each",
                )
                .default("[]"),
            )
            .field(
                FieldDescription::new(
                    "enter",
                    FieldType::List(Box::new(FieldType::Shape("interaction_output"))),
                    "chemistry outputs run on the holder each time the intensity reaches `at` from below",
                )
                .default("null"),
            )
            .sample(r#"{ "at": 1 }"#)
            .roundtrip(round_trip::<BandDef>),
        ComponentDescription::new(EFFECT_FUEL, "A stat of its holder's that a running instance burns.")
            .field(FieldDescription::new(
                "stat",
                FieldType::Text,
                "the stat whose base it drains, `[a-z][a-z0-9_]*`",
            ))
            .field(FieldDescription::new(
                "rate",
                FieldType::F32 {
                    min: Bound::Exclusive(0.0),
                    max: Bound::Open,
                },
                "taken off the base a second; the instance runs out at 0",
            ))
            .sample(r#"{ "stat": "oil", "rate": 1 }"#)
            .roundtrip(round_trip::<FuelDef>),
        ComponentDescription::new(
            EFFECT_EMIT,
            "A chemistry stimulus an instance's holder sends every period of its life to each receiver within the radius but itself.",
        )
        .field(FieldDescription::new(
            "stimulus",
            FieldType::Text,
            "a damage type or a declared tag; the load checks it",
        ))
        .field(FieldDescription::new(
            "amount",
            FieldType::F32 {
                min: Bound::Exclusive(0.0),
                max: Bound::Open,
            },
            "the stimulus's amount, times the instance's factor",
        ))
        .field(FieldDescription::new(
            "radius",
            FieldType::F32 {
                min: Bound::Exclusive(0.0),
                max: Bound::Open,
            },
            "px from the holder's point to a receiver's surface, or to its point without a body",
        ))
        .field(FieldDescription::new(
            "period",
            FieldType::F32 {
                min: Bound::Exclusive(0.0),
                max: Bound::Open,
            },
            "game seconds between emissions, counted as the payload's period is and saved",
        ))
        .sample(r#"{ "stimulus": "heat", "amount": 1, "radius": 50, "period": 0.25 }"#)
        .roundtrip(round_trip::<EmitDef>),
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
                "the stat, `[a-z][a-z0-9_]*`; a target without it takes nothing, unless the line is scoped",
            ))
            .field(FieldDescription::new(
                "modifier",
                FieldType::Shape(MODIFIER),
                "the change; `add` and `pct` scale with the effect's factor; a scoped line may not override",
            ))
            .field(
                FieldDescription::new(
                    "with",
                    FieldType::Filter,
                    "read only where it passes the read's context (a skill's tags, a hit's track, a part's type); the load checks its tags",
                )
                .default("null"),
            )
            .field(
                FieldDescription::new(
                    "while",
                    FieldType::Filter,
                    "read only while it passes the target's own tags, in a read that takes a context; the load checks its tags",
                )
                .default("null"),
            )
            .field(
                FieldDescription::new(
                    "against",
                    FieldType::Filter,
                    "read only at a hit whose other party's tags pass it; the load checks its tags",
                )
                .default("null"),
            )
            .sample(r#"{ "stat": "move_speed", "modifier": { "pct": -0.2 } }"#)
            .roundtrip(round_trip::<StatModifierDef>),
        ComponentDescription::new(
            RESIST_MODIFIER,
            "A modifier on the target's resistance to one damage tag, and on its floor.",
        )
        .field(FieldDescription::new(
            "tag",
            FieldType::Tag,
            "the damage tag; the load checks it",
        ))
        .field(
            FieldDescription::new(
                "modifier",
                FieldType::Shape(MODIFIER),
                "the change, over a base of 1 for a tag the target lists no factor for; a scoped one may not override; one of it and `floor` at least",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "with",
                FieldType::Filter,
                "the modifier reaches only a hit whose context passes it (the part's types, the hit's track, state tags and side); the load checks its tags",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "against",
                FieldType::Filter,
                "the modifier reaches only a hit whose attacker's tags pass it; the load checks its tags",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "floor",
                FieldType::Shape(MODIFIER),
                "a change to the tag's floor, over a base of 0 without one: the lowest a factor above 0 goes in a hit",
            )
            .default("null"),
        )
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
                if let Some(intensity) = instance.intensity {
                    let _ = write!(line, ", intensity {intensity:.2}");
                }
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
