//! Impact damage (7.13c): a body that hurts what it ends a step touching by
//! its closing speed before the step, as a thrown crate, a falling iron
//! block or a rolling boulder does. The physics stops a body at a target's
//! face in the step it arrives, before any contact reports it, and two
//! kinematic bodies never touch, so the kit keeps each body's velocity
//! before the step and measures the touch after it.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::physics::{BodyKind, Collider, Position, RigidBody, Shape, Velocity};
use tungsten_core::save::{RefCodec, RefReader, RefWriter, SaveOptions};
use tungsten_core::{
    Entity, EventQueue, Inspectable, MissingAsset, Plugin, Schedule, SpawnContext, World, system,
};

use crate::damage::health::{Health, Hit};
use crate::damage::kinds::check_type;
use crate::damage::pipeline::{
    DamagePart, DamageRequest, HitPrep, check_hit_prep, hit_prep, refuse_hit_prep,
};
use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, PLATFORMER, TOP_DOWN, describe,
    describe_shape, round_trip,
};
use crate::foundation::inspect::register_rows;
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::saved::order_of;
use crate::foundation::source::Source;
use crate::foundation::tags::{TagFilter, Tags, valid_tag};
use crate::lifecycle::despawn::DESPAWN_OUTSIDE;
use crate::lifecycle::ended::Despawning;
use crate::motion::follow::FOLLOW;
use crate::motion::knockback::KNOCKBACK;
use crate::motion::linear::LINEAR_MOTION;
use crate::motion::orbit::ORBIT;
use crate::motion::oscillator::OSCILLATOR;
use crate::motion::path::PATH_FOLLOW;
use crate::motion::platformer::PLATFORMER_CONTROLLER;
use crate::motion::speed_limit::SPEED_LIMIT;
use crate::motion::stay_in_view::STAY_IN_VIEW;
use crate::motion::steering::STEERING;
use crate::motion::top_down::TOP_DOWN_MOVER;
use crate::motion::wrap::WRAP;
use crate::overlap::hit::DamageDef;
use crate::overlap::pickup::PICKUP_MAGNET;

/// [`ImpactDamagePlugin`]'s record of the step's velocities, in the move
/// phase after the kit's movers and before `physics_step`.
pub const IMPACT_RECORD: &str = "impact_record";

/// [`ImpactDamagePlugin`]'s pass, in the overlap phase after the step.
pub const IMPACT_DAMAGE: &str = "impact_damage";

/// The scene component the plugin registers.
const IMPACT_DAMAGE_COMPONENT: &str = "impact_damage";
/// The shapes of `impact_damage.extra` and `impact_damage.push`.
const IMPACT_EXTRA: &str = "impact_extra";
const IMPACT_PUSH: &str = "impact_push";

/// A hitter: a dynamic body, or a kinematic one with a `mass`, that hurts a
/// target it ends a step touching by how fast the two closed before it.
///
/// Each step its holder's velocity before the step (its record, after the
/// kit's movers, [`step_velocity`]) reaching `speed`, it takes each target
/// whose body ends within `margin` of its own: an entity with [`Health`],
/// alive and not being despawned, whose tags pass `filter`, but itself, its
/// [`Source`] and the entity its hit is credited to. Their closing speed
/// `u` is the two kept velocities' difference along the line between their
/// centres; at `u ≥ speed` it hits once: `amount` when given, else
/// `min(per_mass × mass × (u − speed), most)` and nothing under `floor`,
/// then times `scale`, of the damage type `kind`, with `extra`'s parts while
/// its own tags pass their filter; pushed `away × u` from its centre, at most
/// `most`, plus `along` along its kept velocity; at the target's centre;
/// credited to its credit while that lives, else to its live `Source`; its
/// game's preparation run on it (`prepare`), its crit rolled at its build
/// from its credit's stats; resolved at once with `land` `now`, else queued
/// for the step's resolve. A step's hits build in the targets' save order,
/// then their hitters', and each sends [`Impacted`] with its outcome. `mass`
/// replaces its body's own; a `scale` of 0 leaves it inert.
///
/// Scene name `impact_damage`: `{ "speed": 400, "per_mass": 0.002, "most":
/// 120, "floor": 2, "kind": "damage.physical", "extra": { "when": { "all":
/// ["status.burning"] }, "damage": { "damage.fire": 12 } }, "push": {
/// "away": 0.6, "most": 900 }, "margin": 3, "land": "now", "prepare":
/// "armour" }`, each optional: `speed` 400 px/s, `per_mass` 0.002, no
/// `most`, `floor` 0, the body's `mass`, no `amount`, untyped, no `extra`,
/// no push, every target, `margin` 3 px, `land` `queue`, no preparation and
/// `scale` 1. The load checks its damage types, its tags and its
/// preparation. It is data: a game's save holds it, its `scale` and its
/// credit (by save ID) included.
///
/// # Examples
///
/// ```
/// use tungsten_kit::ImpactDamage;
///
/// let crate_ = ImpactDamage::new().with_floor(2.0).with_most(120.0);
/// assert_eq!(crate_.amount(4.0, 1000.0), 0.002 * 4.0 * 600.0);
/// assert_eq!(crate_.amount(4.0, 600.0), 0.0, "1.6, under the floor");
/// assert_eq!(crate_.amount(4.0, 100_000.0), 120.0);
/// let read: ImpactDamage = serde_json::from_str(r#"{ "floor": 2, "most": 120 }"#).unwrap();
/// assert_eq!(read, crate_);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ImpactFields", into = "ImpactFields")]
pub struct ImpactDamage {
    speed: f32,
    per_mass: f32,
    most: Option<f32>,
    floor: f32,
    mass: Option<f32>,
    amount: Option<f32>,
    kind: Option<String>,
    extra: Option<Box<Extra>>,
    push: Option<Push>,
    filter: TagFilter,
    margin: f32,
    now: bool,
    prepare: Option<String>,
    scale: f32,
    /// Who its hits are credited to, ahead of its `Source`.
    credit: Option<Entity>,
}

/// Parts an impact adds while its holder's tags pass `when`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Extra {
    #[serde(default)]
    when: TagFilter,
    damage: DamageDef,
}

/// An impact's push: `away × u` from the hitter's centre, at most `most`,
/// plus `along` along its kept velocity.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PushFields")]
struct Push {
    away: f32,
    most: Option<f32>,
    along: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PushFields {
    #[serde(default)]
    away: f32,
    #[serde(default)]
    most: Option<f32>,
    #[serde(default)]
    along: f32,
}

impl TryFrom<PushFields> for Push {
    type Error = String;

    fn try_from(fields: PushFields) -> Result<Self, String> {
        for (field, value) in [("away", fields.away), ("along", fields.along)] {
            if !(value.is_finite() && value >= 0.0) {
                return Err(format!("{field}: must be finite and >= 0 (got {value})"));
            }
        }
        if let Some(most) = fields.most
            && !(most.is_finite() && most > 0.0)
        {
            return Err(format!("most: must be finite and > 0 (got {most})"));
        }
        Ok(Self {
            away: fields.away,
            most: fields.most,
            along: fields.along,
        })
    }
}

// The reference is serde's `skip_serializing_if` signature.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(value: &f32) -> bool {
    *value == 0.0
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_three(value: &f32) -> bool {
    *value == MARGIN
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_one(value: &f32) -> bool {
    *value == 1.0
}

/// When an impact's hit resolves, as data writes it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum LandField {
    #[default]
    Queue,
    Now,
}

impl LandField {
    #[allow(clippy::trivially_copy_pass_by_ref)]
    fn is_queue(&self) -> bool {
        *self == LandField::Queue
    }
}

/// The closing speed an impact starts at by default, in px/s.
const SPEED: f32 = 400.0;
/// Its damage a unit of mass a px/s past its speed by default.
const PER_MASS: f32 = 0.002;
/// How near a target its body ends for a touch by default, in px.
const MARGIN: f32 = 3.0;

fn default_speed() -> f32 {
    SPEED
}

fn default_per_mass() -> f32 {
    PER_MASS
}

fn default_margin() -> f32 {
    MARGIN
}

fn default_scale() -> f32 {
    1.0
}

/// An impact rule as data and a save write it, before its values are
/// checked: the save leaves each field out at its default.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ImpactFields {
    #[serde(default = "default_speed")]
    speed: f32,
    #[serde(default = "default_per_mass")]
    per_mass: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    most: Option<f32>,
    #[serde(default, skip_serializing_if = "is_zero")]
    floor: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mass: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    amount: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    extra: Option<Box<Extra>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    push: Option<Push>,
    #[serde(default, skip_serializing_if = "TagFilter::is_everything")]
    filter: TagFilter,
    #[serde(default = "default_margin", skip_serializing_if = "is_three")]
    margin: f32,
    #[serde(default, skip_serializing_if = "LandField::is_queue")]
    land: LandField,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    prepare: Option<String>,
    #[serde(default = "default_scale", skip_serializing_if = "is_one")]
    scale: f32,
}

impl TryFrom<ImpactFields> for ImpactDamage {
    type Error = String;

    fn try_from(fields: ImpactFields) -> Result<Self, String> {
        if !(fields.speed.is_finite() && fields.speed >= 0.0) {
            return Err(format!(
                "speed: must be finite and >= 0 (got {})",
                fields.speed
            ));
        }
        for (field, value) in [
            ("per_mass", fields.per_mass),
            ("floor", fields.floor),
            ("margin", fields.margin),
            ("scale", fields.scale),
        ] {
            if !(value.is_finite() && value >= 0.0) {
                return Err(format!("{field}: must be finite and >= 0 (got {value})"));
            }
        }
        for (field, value) in [
            ("most", fields.most),
            ("mass", fields.mass),
            ("amount", fields.amount),
        ] {
            if let Some(value) = value
                && !(value.is_finite() && value > 0.0)
            {
                return Err(format!("{field}: must be finite and > 0 (got {value})"));
            }
        }
        if let Some(kind) = &fields.kind
            && !valid_tag(kind)
        {
            return Err(format!("kind: `{kind}` is not a damage type name"));
        }
        if let Some(name) = &fields.prepare
            && !crate::foundation::stats::valid_stat(name)
        {
            return Err(format!(
                "prepare: `{name}` is not a preparation name (`[a-z][a-z0-9_]*`)"
            ));
        }
        Ok(Self {
            speed: fields.speed,
            per_mass: fields.per_mass,
            most: fields.most,
            floor: fields.floor,
            mass: fields.mass,
            amount: fields.amount,
            kind: fields.kind,
            extra: fields.extra,
            push: fields
                .push
                .filter(|push| push.away > 0.0 || push.along > 0.0),
            filter: fields.filter,
            margin: fields.margin,
            now: fields.land == LandField::Now,
            prepare: fields.prepare,
            scale: fields.scale,
            credit: None,
        })
    }
}

impl From<ImpactDamage> for ImpactFields {
    fn from(rule: ImpactDamage) -> Self {
        Self {
            speed: rule.speed,
            per_mass: rule.per_mass,
            most: rule.most,
            floor: rule.floor,
            mass: rule.mass,
            amount: rule.amount,
            kind: rule.kind,
            extra: rule.extra,
            push: rule.push,
            filter: rule.filter,
            margin: rule.margin,
            land: if rule.now {
                LandField::Now
            } else {
                LandField::Queue
            },
            prepare: rule.prepare,
            scale: rule.scale,
        }
    }
}

impl Default for ImpactDamage {
    /// [`new`](Self::new).
    fn default() -> Self {
        Self::new()
    }
}

impl ImpactDamage {
    /// A hitter from 400 px/s, 0.002 a unit of mass a px/s past it, with no
    /// cap, no floor, its body's mass, untyped, no push, every target, a
    /// 3 px margin, queued, at scale 1 and credited to its `Source`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            speed: SPEED,
            per_mass: PER_MASS,
            most: None,
            floor: 0.0,
            mass: None,
            amount: None,
            kind: None,
            extra: None,
            push: None,
            filter: TagFilter::default(),
            margin: MARGIN,
            now: false,
            prepare: None,
            scale: 1.0,
            credit: None,
        }
    }

    /// The rule, hurting from a closing speed of `speed` px/s.
    ///
    /// # Panics
    ///
    /// When `speed` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_speed(mut self, speed: f32) -> Self {
        assert!(
            speed.is_finite() && speed >= 0.0,
            "ImpactDamage::with_speed: speed must be finite and >= 0 (got {speed})"
        );
        self.speed = speed;
        self
    }

    /// The rule, dealing `per_mass` a unit of mass a px/s past its speed.
    ///
    /// # Panics
    ///
    /// When `per_mass` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_per_mass(mut self, per_mass: f32) -> Self {
        assert!(
            per_mass.is_finite() && per_mass >= 0.0,
            "ImpactDamage::with_per_mass: per_mass must be finite and >= 0 (got {per_mass})"
        );
        self.per_mass = per_mass;
        self
    }

    /// The rule, dealing at most `most` a hit before its scale.
    ///
    /// # Panics
    ///
    /// When `most` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_most(mut self, most: f32) -> Self {
        assert!(
            most.is_finite() && most > 0.0,
            "ImpactDamage::with_most: most must be finite and > 0 (got {most})"
        );
        self.most = Some(most);
        self
    }

    /// The rule, dealing nothing under `floor`, read before its scale.
    ///
    /// # Panics
    ///
    /// When `floor` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_floor(mut self, floor: f32) -> Self {
        assert!(
            floor.is_finite() && floor >= 0.0,
            "ImpactDamage::with_floor: floor must be finite and >= 0 (got {floor})"
        );
        self.floor = floor;
        self
    }

    /// The rule, hitting as `mass` in place of its body's own: a kinematic
    /// hitter's, or a share of a light body's.
    ///
    /// # Panics
    ///
    /// When `mass` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_mass(mut self, mass: f32) -> Self {
        assert!(
            mass.is_finite() && mass > 0.0,
            "ImpactDamage::with_mass: mass must be finite and > 0 (got {mass})"
        );
        self.mass = Some(mass);
        self
    }

    /// The rule, dealing a flat `amount` a hit before its scale, whatever
    /// the speed past its threshold.
    ///
    /// # Panics
    ///
    /// When `amount` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_amount(mut self, amount: f32) -> Self {
        assert!(
            amount.is_finite() && amount > 0.0,
            "ImpactDamage::with_amount: amount must be finite and > 0 (got {amount})"
        );
        self.amount = Some(amount);
        self
    }

    /// The rule, its hit of the damage type `kind` (`damage.physical`).
    ///
    /// # Panics
    ///
    /// When `kind` is not a tag name.
    #[must_use]
    #[track_caller]
    pub fn with_kind(mut self, kind: &str) -> Self {
        assert!(
            valid_tag(kind),
            "ImpactDamage::with_kind: `{kind}` is not a damage type name"
        );
        self.kind = Some(kind.to_owned());
        self
    }

    /// The rule, adding a part of `amount` of the damage type `kind` to its
    /// hit while its holder's tags pass `when`, whatever its scale.
    ///
    /// # Panics
    ///
    /// When `kind` is not a tag name or `amount` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_extra(mut self, when: TagFilter, kind: &str, amount: f32) -> Self {
        assert!(
            valid_tag(kind) && amount.is_finite() && amount > 0.0,
            "ImpactDamage::with_extra: `{kind}` {amount} is no damage part"
        );
        let parts = std::iter::once((kind.to_owned(), amount)).collect();
        self.extra = Some(Box::new(Extra {
            when,
            damage: DamageDef::Typed(parts),
        }));
        self
    }

    /// The rule, pushing a target `share` of the closing speed away from
    /// its centre, at most `most` px/s when given.
    ///
    /// # Panics
    ///
    /// When `share` is not finite and above 0, or `most` not finite and
    /// above 0.
    #[must_use]
    #[track_caller]
    pub fn with_push_away(mut self, share: f32, most: Option<f32>) -> Self {
        assert!(
            share.is_finite()
                && share > 0.0
                && most.is_none_or(|most| most.is_finite() && most > 0.0),
            "ImpactDamage::with_push_away: share {share} and most {most:?} must be finite and > 0"
        );
        let push = self.push.get_or_insert(Push {
            away: 0.0,
            most: None,
            along: 0.0,
        });
        push.away = share;
        push.most = most;
        self
    }

    /// The rule, pushing a target `velocity` px/s along its holder's kept
    /// velocity.
    ///
    /// # Panics
    ///
    /// When `velocity` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_push_along(mut self, velocity: f32) -> Self {
        assert!(
            velocity.is_finite() && velocity > 0.0,
            "ImpactDamage::with_push_along: velocity must be finite and > 0 (got {velocity})"
        );
        self.push
            .get_or_insert(Push {
                away: 0.0,
                most: None,
                along: 0.0,
            })
            .along = velocity;
        self
    }

    /// The rule, hitting only the targets whose tags pass `filter`.
    #[must_use]
    pub fn with_filter(mut self, filter: TagFilter) -> Self {
        self.filter = filter;
        self
    }

    /// The rule, touching a target whose body ends within `margin` px of
    /// its own.
    ///
    /// # Panics
    ///
    /// When `margin` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_margin(mut self, margin: f32) -> Self {
        assert!(
            margin.is_finite() && margin >= 0.0,
            "ImpactDamage::with_margin: margin must be finite and >= 0 (got {margin})"
        );
        self.margin = margin;
        self
    }

    /// The rule, resolving its hits in its pass, so a game reads their
    /// outcomes from [`Impacted`] in the same step.
    #[must_use]
    pub fn landing_now(mut self) -> Self {
        self.now = true;
        self
    }

    /// The rule, running the game's hit preparation `name`
    /// ([`add_hit_prep`](crate::add_hit_prep)) on each hit.
    ///
    /// # Panics
    ///
    /// When `name` is not `[a-z][a-z0-9_]*`.
    #[must_use]
    #[track_caller]
    pub fn with_prepare(mut self, name: impl Into<String>) -> Self {
        let name = name.into();
        assert!(
            crate::foundation::stats::valid_stat(&name),
            "ImpactDamage::with_prepare: `{name}` is not a preparation name"
        );
        self.prepare = Some(name);
        self
    }

    /// The rule at `scale` ([`set_scale`](Self::set_scale)).
    #[must_use]
    #[track_caller]
    pub fn with_scale(mut self, scale: f32) -> Self {
        self.set_scale(scale);
        self
    }

    /// Multiplies its hits' amount by `scale`, after the floor, its extra
    /// parts aside: a thrower's factor; 0 leaves it inert.
    ///
    /// # Panics
    ///
    /// When `scale` is not finite and at least 0.
    #[track_caller]
    pub fn set_scale(&mut self, scale: f32) {
        assert!(
            scale.is_finite() && scale >= 0.0,
            "ImpactDamage::set_scale: scale must be finite and >= 0 (got {scale})"
        );
        self.scale = scale;
    }

    /// Credits its hits to `credit` while that lives, ahead of its
    /// `Source`; it never hits the entity it credits.
    pub fn set_credit(&mut self, credit: Option<Entity>) {
        self.credit = credit;
    }

    /// Hits only the targets whose tags pass `filter`.
    pub fn set_filter(&mut self, filter: TagFilter) {
        self.filter = filter;
    }

    /// Its scale.
    #[must_use]
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// The entity it credits, ahead of its `Source`, when set.
    #[must_use]
    pub fn credit(&self) -> Option<Entity> {
        self.credit
    }

    /// The mass it hits as in place of its body's, when set.
    #[must_use]
    pub fn mass(&self) -> Option<f32> {
        self.mass
    }

    /// What its rule deals a hitter of `mass` closing at `closing` px/s,
    /// before its scale: nothing under its speed; its flat amount when it
    /// has one; else `min(per_mass × mass × (closing − speed), most)`, and
    /// nothing under its floor.
    #[must_use]
    pub fn amount(&self, mass: f32, closing: f32) -> f32 {
        if closing.is_nan() || closing < self.speed {
            return 0.0;
        }
        if let Some(flat) = self.amount {
            return flat;
        }
        let amount = self.per_mass * mass * (closing - self.speed);
        let amount = match self.most {
            Some(most) => amount.min(most),
            None => amount,
        };
        if amount < self.floor { 0.0 } else { amount }
    }

    /// The load's check: its kind, its extra's damage types and tags, its
    /// filter's tags and its preparation.
    fn check(&self, world: &World) -> Result<(), MissingAsset> {
        if let Some(kind) = &self.kind {
            check_type(world, "kind", kind, false)?;
        }
        if let Some(extra) = &self.extra {
            extra.damage.check(world)?;
            extra.when.check(world, "extra.when")?;
        }
        self.filter.check(world, "filter")?;
        if let Some(name) = &self.prepare {
            check_hit_prep(world, "prepare", name)?;
        }
        Ok(())
    }
}

impl Inspectable for ImpactDamage {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        let amount = self.amount.map_or_else(
            || format!("{} a mass a px/s past {}", self.per_mass, self.speed),
            |flat| format!("{flat} past {}", self.speed),
        );
        vec![
            ("amount", amount),
            ("scale", self.scale.to_string()),
            (
                "credit",
                self.credit
                    .map_or_else(|| "its source".to_string(), |credit| format!("{credit:?}")),
            ),
        ]
    }
}

/// [`ImpactDamage`] as a save writes it: the rule, its credit by save ID.
fn impact_saved() -> SaveOptions<ImpactDamage> {
    SaveOptions::<ImpactDamage>::new().refs(RefCodec {
        save: |rule, out: &mut RefWriter<'_>| {
            if let Some(credit) = rule.credit {
                out.put("credit", credit);
            }
        },
        load: |rule, refs: &RefReader<'_>| {
            if let Some(credit) = refs.first("credit") {
                rule.credit = Some(credit);
            }
        },
        keys: |rule| {
            rule.credit
                .map(|_| vec!["credit".to_owned()])
                .unwrap_or_default()
        },
    })
}

/// A hitter met a target: the plain event [`ImpactDamage`] sends for each
/// hit, with its outcome when the hit landed in the pass (`land` `now`),
/// so a game ties a kill to the body that dealt it. Build one with
/// [`Impacted::new`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Impacted {
    /// The body that hit.
    pub hitter: Entity,
    /// What it hit.
    pub target: Entity,
    /// The hit's outcome, for one resolved in the pass; `None` for one
    /// queued for the step's resolve.
    pub outcome: Option<Hit>,
}

impl Impacted {
    /// An event with these fields.
    #[must_use]
    pub fn new(hitter: Entity, target: Entity, outcome: Option<Hit>) -> Self {
        Self {
            hitter,
            target,
            outcome,
        }
    }
}

/// The velocities [`IMPACT_RECORD`] kept this step, by entity index: a
/// World resource it inserts, which no save holds.
#[derive(Debug, Default)]
struct ImpactRecord {
    kept: Vec<(Entity, Vec2)>,
}

/// `entity`'s velocity as [`IMPACT_RECORD`] kept it this step, after the
/// kit's movers and before the physics step: each [`ImpactDamage`]
/// holder's and each [`Health`] holder's while any hitter exists; `None`
/// for one it did not keep. A game's own velocity change meant for the
/// step's impacts runs before [`IMPACT_RECORD`] by name.
#[must_use]
pub fn step_velocity(world: &World, entity: Entity) -> Option<Vec2> {
    let kept = &world.get_resource::<ImpactRecord>()?.kept;
    let at = kept
        .binary_search_by_key(&entity.id(), |(kept, _)| kept.id())
        .ok()?;
    (kept[at].0 == entity).then_some(kept[at].1)
}

/// [`IMPACT_RECORD`]: each hitter's and each `Health` holder's velocity,
/// while a hitter exists.
fn record(world: &mut World) {
    if !world.has_any::<ImpactDamage>() {
        if let Some(record) = world.get_resource_mut::<ImpactRecord>() {
            record.kept.clear();
        }
        return;
    }
    let mut record = world.remove_resource::<ImpactRecord>().unwrap_or_default();
    record.kept.clear();
    record.kept.extend(
        world
            .query::<(Entity, &Velocity, &ImpactDamage)>()
            .map(|(entity, velocity, _)| (entity, velocity.0)),
    );
    record.kept.extend(
        world
            .query::<(Entity, &Velocity, &Health)>()
            .filter(|&(entity, ..)| !world.has::<ImpactDamage>(entity))
            .map(|(entity, velocity, _)| (entity, velocity.0)),
    );
    record.kept.sort_unstable_by_key(|(entity, _)| entity.id());
    world.insert_resource(record);
}

/// The gap between two bodies' surfaces, boxes or circles about their
/// centres: below 0 where they overlap; a box against a box by its largest
/// axis.
fn gap(a: Vec2, a_shape: Shape, b: Vec2, b_shape: Shape) -> f32 {
    let half = |shape: Shape| shape.half_extents();
    match (a_shape, b_shape) {
        (Shape::Circle { radius: ra }, Shape::Circle { radius: rb }) => a.distance(b) - ra - rb,
        (Shape::Circle { radius }, other) | (other, Shape::Circle { radius }) => {
            let (circle, boxed) = if matches!(a_shape, Shape::Circle { .. }) {
                (a, b)
            } else {
                (b, a)
            };
            ((circle - boxed).abs() - half(other))
                .max(Vec2::ZERO)
                .length()
                - radius
        }
        _ => ((a - b).abs() - (half(a_shape) + half(b_shape))).max_element(),
    }
}

/// A target the pass measures: its entity, centre and shape.
type Target = (Entity, Vec2, Shape);

/// A touch the pass found: the hitter, the target, the closing speed and
/// the hitter's mass, centre and kept velocity.
#[derive(Clone, Copy)]
struct Touch {
    hitter: Entity,
    target: Entity,
    closing: f32,
    mass: f32,
    centre: Vec2,
    at: Vec2,
    kept: Vec2,
    credit: Option<Entity>,
}

/// [`IMPACT_DAMAGE`]'s lists, kept so a warm step allocates nothing.
#[derive(Default)]
struct ImpactRunner {
    targets: Vec<Target>,
    touches: Vec<Touch>,
}

/// A body's mass as an impact reads it: its rule's, else a dynamic body's
/// own; `None` for a hitter without one.
fn hitter_mass(rule: &ImpactDamage, body: Option<&RigidBody>) -> Option<f32> {
    rule.mass.or_else(|| {
        body.filter(|body| body.kind == BodyKind::Dynamic && body.inv_mass > 0.0)
            .map(|body| 1.0 / body.inv_mass)
    })
}

/// A body's kept velocity: its record, else its velocity now.
fn kept_velocity(world: &World, entity: Entity) -> Vec2 {
    step_velocity(world, entity)
        .or_else(|| world.get::<Velocity>(entity).map(|velocity| velocity.0))
        .unwrap_or(Vec2::ZERO)
}

impl ImpactRunner {
    fn run(&mut self, world: &mut World) {
        if !world.has_any::<ImpactDamage>() {
            return;
        }
        self.targets.clear();
        self.targets.extend(
            world
                .query::<(Entity, &Health, &Position, &Collider)>()
                .filter(|(entity, health, ..)| {
                    !health.is_dead() && !world.has::<Despawning>(*entity)
                })
                .map(|(entity, _, position, collider)| {
                    (entity, position.0 + collider.offset, collider.shape)
                }),
        );
        if self.targets.is_empty() {
            return;
        }
        self.touches.clear();
        for (hitter, rule, position, collider, body) in world.query::<(
            Entity,
            &ImpactDamage,
            &Position,
            &Collider,
            Option<&RigidBody>,
        )>() {
            if rule.scale <= 0.0 || world.has::<Despawning>(hitter) {
                continue;
            }
            let Some(mass) = hitter_mass(rule, body) else {
                continue;
            };
            let kept = kept_velocity(world, hitter);
            if kept.length_squared() < rule.speed * rule.speed {
                continue;
            }
            let centre = position.0 + collider.offset;
            let source = world.get::<Source>(hitter).map(|source| source.entity());
            let credit = rule
                .credit
                .filter(|&credit| world.is_alive(credit))
                .or_else(|| source.filter(|&source| world.is_alive(source)));
            for &(target, at, shape) in &self.targets {
                if target == hitter || Some(target) == source || Some(target) == credit {
                    continue;
                }
                if gap(centre, collider.shape, at, shape) > rule.margin
                    || !rule.filter.matches(world.get::<Tags>(target))
                {
                    continue;
                }
                let towards = (at - centre).normalize_or_zero();
                let closing = (kept - kept_velocity(world, target)).dot(towards);
                if closing >= rule.speed {
                    self.touches.push(Touch {
                        hitter,
                        target,
                        closing,
                        mass,
                        centre,
                        at,
                        kept,
                        credit,
                    });
                }
            }
        }
        if self.touches.is_empty() {
            return;
        }
        let order = order_of(world);
        self.touches
            .sort_by_key(|touch| (order.key(touch.target), order.key(touch.hitter)));
        let touches = std::mem::take(&mut self.touches);
        for touch in &touches {
            land(world, touch);
        }
        self.touches = touches;
        self.touches.clear();
    }
}

/// Builds and lands `touch`'s hit, and sends [`Impacted`].
fn land(world: &mut World, touch: &Touch) {
    let Some(rule) = world.get::<ImpactDamage>(touch.hitter) else {
        return;
    };
    let amount = rule.amount(touch.mass, touch.closing) * rule.scale;
    if amount <= 0.0 {
        return;
    }
    let prep: Option<HitPrep> = match rule.prepare.as_deref() {
        None => None,
        Some(name) => {
            let Some(prep) = hit_prep(world, name) else {
                let name = name.to_owned();
                refuse_hit_prep(world, "an impact", &name);
                return;
            };
            Some(prep)
        }
    };
    let main = match &rule.kind {
        Some(kind) => DamagePart::new(amount).with_tags(Tags::new([kind.as_str()])),
        None => DamagePart::new(amount),
    };
    let mut parts = vec![main];
    if let Some(extra) = &rule.extra
        && extra.when.matches(world.get::<Tags>(touch.hitter))
    {
        match extra.damage.parts() {
            Some(typed) => parts.extend(typed),
            None => parts.push(DamagePart::new(extra.damage.total())),
        }
    }
    let push = rule.push.map(|push| {
        let mut velocity = Vec2::ZERO;
        if push.away > 0.0 {
            let away = push.away * touch.closing;
            let away = push.most.map_or(away, |most| away.min(most));
            velocity = (touch.at - touch.centre).normalize_or_zero() * away;
        }
        if push.along > 0.0 {
            velocity += touch.kept.normalize_or_zero() * push.along;
        }
        velocity
    });
    let now = rule.now;
    let mut request = DamageRequest::from_parts(touch.target, parts).with_position(touch.at);
    if let Some(credit) = touch.credit {
        request = request.with_source(credit);
    }
    if let Some(push) = push {
        request = request.with_knockback(push);
    }
    if let Some(prep) = prep {
        prep(world, &mut request);
    }
    let request = request.roll_crit(world);
    let outcome = if now {
        Some(request.apply(world))
    } else {
        request.queue(world);
        None
    };
    if let Some(queue) = world.get_resource_mut::<EventQueue<Impacted>>() {
        queue.send(Impacted::new(touch.hitter, touch.target, outcome));
    }
}

/// `components.impact_damage` as a scene writes it: read as the rule
/// reads, written with every field, each default included.
struct ImpactDamageDef(ImpactDamage);

impl<'de> Deserialize<'de> for ImpactDamageDef {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        ImpactDamage::deserialize(deserializer).map(Self)
    }
}

impl Serialize for ImpactDamageDef {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let rule = &self.0;
        SceneFields {
            speed: rule.speed,
            per_mass: rule.per_mass,
            most: rule.most,
            floor: rule.floor,
            mass: rule.mass,
            amount: rule.amount,
            kind: rule.kind.as_deref(),
            extra: rule.extra.as_deref(),
            push: rule.push,
            filter: &rule.filter,
            margin: rule.margin,
            land: if rule.now {
                LandField::Now
            } else {
                LandField::Queue
            },
            prepare: rule.prepare.as_deref(),
            scale: rule.scale,
        }
        .serialize(serializer)
    }
}

/// Every field of a rule, for [`ImpactDamageDef`]'s writing.
#[derive(Serialize)]
struct SceneFields<'a> {
    speed: f32,
    per_mass: f32,
    most: Option<f32>,
    floor: f32,
    mass: Option<f32>,
    amount: Option<f32>,
    kind: Option<&'a str>,
    extra: Option<&'a Extra>,
    push: Option<Push>,
    filter: &'a TagFilter,
    margin: f32,
    land: LandField,
    prepare: Option<&'a str>,
    scale: f32,
}

/// Registers the `impact_damage` scene component and its saved form,
/// `EventQueue<Impacted>`, [`IMPACT_RECORD`] in the move phase after the
/// kit's movers, the speed limit and the pickups' magnet, each when
/// registered, and [`IMPACT_DAMAGE`] in the overlap phase, before
/// `despawn_outside`, `wrap` and `stay_in_view`, each when registered. Both
/// return at once while no entity holds an [`ImpactDamage`].
///
/// # Examples
///
/// ```
/// use tungsten_core::{Plugin, Schedule, Stage, World};
/// use tungsten_kit::{IMPACT_DAMAGE, IMPACT_RECORD, ImpactDamagePlugin};
///
/// let mut world = World::new();
/// let mut schedule = Schedule::new();
/// ImpactDamagePlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
/// assert_eq!(schedule.stage_of(IMPACT_RECORD), Some(Stage::FixedUpdate));
/// assert_eq!(schedule.stage_of(IMPACT_DAMAGE), Some(Stage::FixedUpdate));
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct ImpactDamagePlugin;

impl Plugin for ImpactDamagePlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<ImpactDamage>(world, "ImpactDamage");
        world.register_saved_with(IMPACT_DAMAGE_COMPONENT, impact_saved());
        describe_impact(world);
        world.register_event::<Impacted>();
        world.register_component_with_assets(
            IMPACT_DAMAGE_COMPONENT,
            |def: &ImpactDamageDef, world: &World| def.0.check(world),
            |def: ImpactDamageDef, _: &SpawnContext<'_>| (def.0,),
        );
        if schedule.contains(IMPACT_RECORD) {
            return;
        }
        schedule.add(
            Phase::Move.stage(),
            Phase::Move
                .join(system(IMPACT_RECORD, record))
                .after_if_present(PATH_FOLLOW)
                .after_if_present(LINEAR_MOTION)
                .after_if_present(OSCILLATOR)
                .after_if_present(PLATFORMER_CONTROLLER)
                .after_if_present(TOP_DOWN_MOVER)
                .after_if_present(STEERING)
                .after_if_present(FOLLOW)
                .after_if_present(ORBIT)
                .after_if_present(KNOCKBACK)
                .after_if_present(PICKUP_MAGNET)
                .after_if_present(SPEED_LIMIT),
        );
        let mut runner = ImpactRunner::default();
        schedule.add(
            Phase::Overlap.stage(),
            Phase::Overlap
                .join(system(IMPACT_DAMAGE, move |world: &mut World| {
                    runner.run(world);
                }))
                .before_if_present(DESPAWN_OUTSIDE)
                .before_if_present(WRAP)
                .before_if_present(STAY_IN_VIEW),
        );
    }
}

/// The non-negative amount fields' type.
fn amount() -> FieldType {
    FieldType::F32 {
        min: Bound::Inclusive(0.0),
        max: Bound::Open,
    }
}

/// The positive amount fields' type.
fn positive() -> FieldType {
    FieldType::F32 {
        min: Bound::Exclusive(0.0),
        max: Bound::Open,
    }
}

fn describe_impact(world: &mut World) {
    describe(
        world,
        ComponentDescription::new(
            IMPACT_DAMAGE_COMPONENT,
            "Hurts what its body ends a step touching by their closing speed before it: a thrown crate, a falling block, a rolling boulder.",
        )
        .field(
            FieldDescription::new(
                "speed",
                amount(),
                "the closing speed in px/s it hurts from, along the line between the centres, from both bodies' velocities before the step",
            )
            .default("400.0"),
        )
        .field(
            FieldDescription::new(
                "per_mass",
                amount(),
                "its damage a unit of mass a px/s past its speed",
            )
            .default("0.002"),
        )
        .field(
            FieldDescription::new("most", positive(), "the most a hit deals before its scale")
                .default("null"),
        )
        .field(
            FieldDescription::new(
                "floor",
                amount(),
                "a hit under it deals nothing, read before its scale",
            )
            .default("0.0"),
        )
        .field(
            FieldDescription::new(
                "mass",
                positive(),
                "the mass it hits as in place of its body's own: a kinematic hitter's, a light body's share; a dynamic body's own when left out",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "amount",
                positive(),
                "a flat amount a hit in place of its speed's, before its scale",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "kind",
                FieldType::Text,
                "its hit's damage type; untyped when left out",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "extra",
                FieldType::Shape(IMPACT_EXTRA),
                "parts its hit adds while its own tags pass a filter, whatever its scale",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "push",
                FieldType::Shape(IMPACT_PUSH),
                "the knockback its hit gives; none when left out",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new("filter", FieldType::Filter, "the tags a target passes")
                .default("{}"),
        )
        .field(
            FieldDescription::new(
                "margin",
                amount(),
                "how near a target its body ends for a touch, in px",
            )
            .default("3.0"),
        )
        .field(
            FieldDescription::new(
                "land",
                FieldType::Enum(&["queue", "now"]),
                "`now` resolves its hits in its pass, each `Impacted` with its outcome; `queue` at the step's resolve",
            )
            .default("\"queue\""),
        )
        .field(
            FieldDescription::new(
                "prepare",
                FieldType::Text,
                "a game's hit preparation by name, run on each hit before its crit's roll; the load refuses one no game registered",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "scale",
                amount(),
                "what its hits' amount is multiplied by after the floor, a thrower's factor; 0 leaves it inert",
            )
            .default("1.0"),
        )
        .event("Impacted")
        .genre(PLATFORMER)
        .genre(TOP_DOWN)
        .sample("{}")
        .roundtrip(round_trip::<ImpactDamageDef>),
    );
    describe_shape(
        world,
        ComponentDescription::new(
            IMPACT_EXTRA,
            "Parts an impact adds while its hitter's tags pass a filter.",
        )
        .field(
            FieldDescription::new("when", FieldType::Filter, "the hitter's tags it needs")
                .default("{}"),
        )
        .field(FieldDescription::new(
            "damage",
            FieldType::Damage,
            "the parts, a number or declared damage types to amounts",
        ))
        .sample(r#"{ "damage": { "damage.fire": 12 } }"#)
        .roundtrip(round_trip::<Extra>),
    );
    describe_shape(
        world,
        ComponentDescription::new(
            IMPACT_PUSH,
            "An impact's knockback: a share of the closing speed away from the hitter, and a speed along its flight.",
        )
        .field(
            FieldDescription::new(
                "away",
                amount(),
                "times the closing speed, away from the hitter's centre",
            )
            .default("0.0"),
        )
        .field(
            FieldDescription::new("most", positive(), "the most the `away` push gives, in px/s")
                .default("null"),
        )
        .field(
            FieldDescription::new(
                "along",
                amount(),
                "px/s along the hitter's velocity before the step",
            )
            .default("0.0"),
        )
        .sample(r#"{ "away": 0.6 }"#)
        .roundtrip(round_trip::<Push>),
    );
}

#[cfg(test)]
#[path = "../tests/impact.rs"]
mod tests;
