//! The damage pipeline: requests of typed parts that carry a source, a
//! position, a knockback, a crit flag and a periodic flag, resolved through
//! conversion, penetration and resistances per part, then once through a
//! shield and health, at once or once a step in sorted order; heals the
//! same way.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::save::{RefCodec, RefReader, RefWriter, SaveOptions};
use tungsten_core::{Entity, EventQueue, MissingAsset, World};

use crate::damage::defense::{Resist, Shield};
use crate::damage::health::{
    Damaged, DamagedPart, Health, Hit, ImmunePart, disarm, start_flash_as,
};
use crate::damage::kinds::{
    Conversion, ConversionRule, DamageKinds, DamageTypeTable, Penetration, merge_types,
};
use crate::foundation::explain::{
    CritRecord, ExplainRecord, HitPartRecord, HitRecord, KitExplain, ResistRecord,
};
use crate::foundation::lines::{Read, TagSet};
use crate::foundation::rng::KitRng;
use crate::foundation::saved::order_of;
use crate::foundation::stats::Stats;
use crate::foundation::tags::{SIDE_FRONT, SIDE_REAR, TRACK_DOT, TRACK_HIT, Tags};
use crate::foundation::trace::{TraceKind, trace};
use crate::intent::aim::Aim;
use crate::intent::sheet::Delivery;
use crate::lifecycle::ended::Despawning;
use crate::motion::knockback::knock_back;

/// [`HealthPlugin`](crate::HealthPlugin)'s system that resolves the queued
/// requests, in the resolve phase.
pub const DAMAGE_RESOLVE: &str = "damage_resolve";

/// One part of a hit: an amount and its damage types, which conversion,
/// penetration and a [`Resist`] read. A part without types takes its
/// request's ([`DamageRequest::with_tags`]); a part of 0 drops before any
/// arithmetic.
///
/// # Examples
///
/// ```
/// use tungsten_kit::{DamagePart, Tags};
///
/// let slash = DamagePart::new(8.0).with_tags(Tags::new(["physical.slash"]));
/// assert_eq!(slash.amount(), 8.0);
/// assert!(slash.tags().has("physical"), "a type carries its ancestors");
/// assert!(DamagePart::new(2.0).tags().is_empty(), "untyped");
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct DamagePart {
    tags: Tags,
    amount: f32,
}

impl DamagePart {
    /// An untyped part of `amount`.
    ///
    /// # Panics
    ///
    /// When `amount` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn new(amount: f32) -> Self {
        assert!(
            amount.is_finite() && amount >= 0.0,
            "DamagePart::new: amount must be finite and >= 0 (got {amount})"
        );
        Self {
            tags: Tags::default(),
            amount,
        }
    }

    /// The part, of the damage types `tags` (`physical.slash`).
    #[must_use]
    pub fn with_tags(mut self, tags: Tags) -> Self {
        self.tags = tags;
        self
    }

    /// Its amount before resistances.
    #[must_use]
    pub fn amount(&self) -> f32 {
        self.amount
    }

    /// Its damage types; empty for an untyped part.
    #[must_use]
    pub fn tags(&self) -> &Tags {
        &self.tags
    }
}

/// A request's parts: one, as every request built by
/// [`DamageRequest::new`] has, kept without a list, or a list.
#[derive(Debug, Clone)]
enum Parts {
    One(DamagePart),
    Many(Vec<DamagePart>),
}

impl Parts {
    fn as_slice(&self) -> &[DamagePart] {
        match self {
            Parts::One(part) => std::slice::from_ref(part),
            Parts::Many(parts) => parts,
        }
    }
}

impl PartialEq for Parts {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

/// A hit to resolve: its target and its parts, each an amount with its
/// damage types, with the entity it came from, the types its untyped parts
/// take (`damage.fire`), where it landed, the push it gives, whether it was
/// a critical one, whether it is periodic (an effect's payload, which
/// passes the target's invulnerability window and starts none), and its
/// own [`Penetration`] and [`Conversion`]. The kit's hitboxes, contact
/// damage, projectiles and effects queue one per hit; a game's Rust builds
/// its own and either [`apply`](Self::apply)s it now or
/// [`queue`](Self::queue)s it for the step's [`DAMAGE_RESOLVE`].
///
/// Its crit is decided once: by the game ([`with_crit`](Self::with_crit),
/// carried with no draw and no factor); by its builder
/// ([`roll_crit`](Self::roll_crit), from its source's stats); or, when
/// neither did, at the resolve, below.
///
/// Resolving one, now or queued:
///
/// 0. Crit: a request no rule decided, not periodic, whose source's
///    [`Stats::CRIT_CHANCE`] is above 0, draws once from the [`KitRng`],
///    first, ignored or not, in the resolve's order; under the chance it
///    is a crit, and every part is multiplied by the source's
///    [`Stats::CRIT_MULTIPLIER`], 1.5 without one, before the conversion
///    and the defences. A periodic request never crits.
/// 1. Each part takes its own types, else the request's; a part of 0 drops.
///    No part left, or a target that is gone, being despawned by the kit,
///    without [`Health`], dead or, unless the request is periodic, inside
///    its invulnerability window, is [`Hit::Ignored`], with no side effect.
/// 2. Dealt damage, then conversion: a request that is not periodic, whose
///    source lives and holds [`Stats::DAMAGE_DEALT`], has every part
///    multiplied by that stat read in the hit's context (its context,
///    track, state tags and side, with the source's tags for `while` lines
///    and the target's for `against` lines), at least 0, so its landed and
///    immune amounts and its damage-over-time base carry it; a crit its
///    builder decided multiplies with it. Then the hit's own rules, then
///    its source's [`Conversion`], in one pass, each share a new part of
///    its rule's type alone; parts of equal types then merge in
///    first-appearance order.
/// 3. Penetration: each part's points, the hit's own [`Penetration`]'s and
///    its source's matching entries, summed and capped at 1; a periodic
///    request takes none.
/// 4. The target's defences, per part, read in the hit's context: the
///    request's context and state tags, its track (`delivery.hit`, or
///    `delivery.dot` for a periodic one), its side (`side.front` or
///    `side.rear`) and the part's types, with the target's tags for `while`
///    lines and the source's for `against` lines. Each of the target's
///    [`Resist`] factors whose tag the part carries, with its scoped lines
///    that pass: one of 0 drops the part before any product, and an
///    [`ImmunePart`] goes out for it with its amount before the resistances
///    and its [`DamageKinds`]; each other rises to its tag's floor, then one
///    in `(0, 1)` gains the part's points, capped at 1; the factors and the
///    target's [`Stats::DAMAGE_TAKEN`], read in the same context, multiply
///    the part in `f64`, clamped to `f32::MAX`, and a result of 0 drops the
///    part.
/// 5. No part left is [`Hit::Ignored`], with no side effect but step 4's
///    [`ImmunePart`]s.
/// 6. The parts' total falls once: its [`Shield`] takes what it can and
///    waits its delay again, health falls by the rest, and the window and
///    the [`HitFlash`](crate::HitFlash) start, unless the request is
///    periodic.
/// 7. A [`DamagedPart`] goes out per part, in part order, with its amount
///    after the resistances and its [`DamageKinds`], then one [`Damaged`]
///    with the total and the parts' kinds together, each with the source,
///    the position, the periodic flag and the crit flag; the trace records
///    each part, the hit and any kill.
/// 8. The outcome is [`Hit::Killed`], [`Hit::Hurt`] or [`Hit::Absorbed`]
///    (the shield took it all), and on the last two the knockback goes
///    through [`knock_back`](crate::knock_back). A kill of a health with a
///    death policy starts its delay ([`Health::with_despawn_on_death`]).
///
/// A one-part request with no conversion, no penetration and no modifier
/// on the target's [`Resist`] resolves as an untyped hit always has.
///
/// A hit's side: a kit hit takes it at its strike, from the blow's
/// direction against the target's [`Aim`]; a game's
/// [`with_direction`](Self::with_direction) is judged against the target's
/// aim at the resolve. A hit travelling against the aim lands on
/// [`Side::Front`], else [`Side::Rear`]; no direction or no aim, no side.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_core::{EventQueue, Plugin, Schedule, Stage, Time, World};
/// use tungsten_kit::{DamageRequest, Damaged, Health, HealthPlugin, Hit, Resist, Tags};
///
/// let mut world = World::new();
/// world.insert_resource(Time::new());
/// let mut schedule = Schedule::new();
/// HealthPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// let mage = world.spawn();
/// let golem = world.spawn_with((Health::new(10.0), Resist::new().with_factor("damage.fire", 0.5)));
/// let fireball = DamageRequest::new(golem, 4.0)
///     .with_source(mage)
///     .with_tags(Tags::new(["damage.fire"]))
///     .with_position(Vec2::new(12.0, 3.0));
/// assert_eq!(fireball.clone().apply(&mut world), Hit::Hurt);
/// assert_eq!(world.get::<Health>(golem).unwrap().current(), 8.0, "half resisted");
/// let sent = world.resource::<EventQueue<Damaged>>().iter().next().copied().unwrap();
/// assert_eq!((sent.amount, sent.source, sent.position), (2.0, Some(mage), Some(Vec2::new(12.0, 3.0))));
///
/// // Queued, it waits for the step's resolve.
/// assert!(fireball.queue(&mut world));
/// assert_eq!(world.get::<Health>(golem).unwrap().current(), 8.0);
/// world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
/// schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
/// assert_eq!(world.get::<Health>(golem).unwrap().current(), 6.0);
/// ```
///
/// Typed parts, converted and resisted each:
///
/// ```
/// use tungsten_core::{EventQueue, Plugin, Schedule, World};
/// use tungsten_kit::{
///     Conversion, DamagePart, DamageRequest, DamagedPart, Health, HealthPlugin, Resist, Tags,
/// };
///
/// let mut world = World::new();
/// let mut schedule = Schedule::new();
/// HealthPlugin.build(&mut schedule, &mut world);
///
/// let golem = world.spawn_with((Health::new(20.0), Resist::new().with_factor("cold", 0.5)));
/// let hit = DamageRequest::from_parts(
///     golem,
///     [DamagePart::new(8.0).with_tags(Tags::new(["slash"])), DamagePart::new(2.0)],
/// )
/// .with_tags(Tags::new(["blunt"]))
/// .with_conversion(Conversion::new().with_rule("slash", "cold", 0.5));
/// assert_eq!(hit.amount(), 10.0);
/// hit.apply(&mut world);
/// let parts: Vec<f32> = world.resource::<EventQueue<DamagedPart>>().iter().map(|p| p.amount).collect();
/// assert_eq!(parts, [4.0, 2.0, 2.0], "slash 4, cold 4 halved, the untyped part as `blunt`");
/// assert_eq!(world.get::<Health>(golem).unwrap().current(), 12.0);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct DamageRequest {
    target: Entity,
    parts: Parts,
    source: Option<Entity>,
    tags: Tags,
    position: Option<Vec2>,
    knockback: Option<Vec2>,
    crit: Crit,
    /// Periodic, pushing always and typed, one bit each in one byte, so
    /// the depth takes the byte the flags freed.
    flags: HitFlags,
    /// The side a kit hit took at its strike, in the byte beside the flags.
    side: Option<Side>,
    penetration: Option<Penetration>,
    /// Its own conversion, or the box of its rarer parts, which holds the
    /// conversion too: one slot, so a request keeps its size.
    own: Option<Own>,
    /// How many chemistry reactions led to it, carried onto its parts, in
    /// the byte beside the flags.
    depth: u8,
}

/// A request's flags, a bit each: periodic (an effect's payload), pushing
/// always (its knockback lands where the hit is ignored for the target's
/// window too: contact damage's `push_always`), typed (at a status's
/// application, so the source's conversion leaves it as it is) and lethal
/// ([`kill`](crate::kill)'s: all the health, past every defence).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct HitFlags(u8);

impl HitFlags {
    const PERIODIC: u8 = 1;
    const PUSH_ALWAYS: u8 = 1 << 1;
    const TYPED: u8 = 1 << 2;
    const LETHAL: u8 = 1 << 3;

    fn has(self, flag: u8) -> bool {
        self.0 & flag != 0
    }

    fn set(&mut self, flag: u8, on: bool) {
        if on {
            self.0 |= flag;
        } else {
            self.0 &= !flag;
        }
    }
}

/// The tags of a request without a context or state tags.
static NO_TAGS: Tags = Tags::EMPTY;

/// How far a request's crit is decided, in a flag's byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Crit {
    /// No rule decided it: the resolve draws from its source's stats.
    Open,
    /// Decided: no crit.
    No,
    /// Decided: a crit, whose factor is the game's or already in the
    /// parts.
    Yes,
}

/// The multiplier a crit takes without a `crit_multiplier` stat.
const CRIT_MULTIPLIER: f32 = 1.5;

/// The crit chance and multiplier of `source`'s entity-level stats, when
/// the chance is above 0.
fn crit_odds(world: &World, source: Option<Entity>) -> Option<(f32, f32)> {
    let stats = world.get::<Stats>(source?)?;
    let chance = stats
        .get(Stats::CRIT_CHANCE)
        .filter(|&chance| chance > 0.0)?;
    let multiplier = stats.get(Stats::CRIT_MULTIPLIER).unwrap_or(CRIT_MULTIPLIER);
    Some((chance.min(1.0), multiplier))
}

/// One draw of the kit's stream, a crit's roll: it crits under the chance.
/// The stream is the World's, or a default one inserted.
fn draw_roll(world: &mut World) -> f32 {
    if !world.has_resource::<KitRng>() {
        world.insert_resource(KitRng::default());
    }
    world.resource_mut::<KitRng>().unit()
}

/// A request's own conversion alone, or its boxed extras.
#[derive(Debug, Clone, PartialEq)]
enum Own {
    Conversion(Conversion),
    Extras(Box<Extras>),
}

/// What only some requests carry, boxed so the rest stay small: the
/// conversion, a context, state tags and a direction.
#[derive(Debug, Clone, Default, PartialEq)]
struct Extras {
    conversion: Option<Conversion>,
    context: Tags,
    state: Tags,
    direction: Option<Vec2>,
    /// The skill's snapshot a delivery's hit resolves from.
    sheet: Option<Delivery>,
}

/// The side of a target a hit lands on, judged from the way the hit
/// travels against the way the target aims ([`Aim`]).
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_kit::Side;
///
/// // A target aiming right, hit by a blow travelling left: from the front.
/// assert_eq!(Side::of(Vec2::NEG_X, Vec2::X), Some(Side::Front));
/// assert_eq!(Side::of(Vec2::X, Vec2::X), Some(Side::Rear));
/// assert_eq!(Side::of(Vec2::ZERO, Vec2::X), None);
/// assert_eq!(Side::Front.tag(), "side.front");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    /// Against the target's aim.
    Front,
    /// From behind or beside it.
    Rear,
}

impl Side {
    /// The side a hit travelling along `direction` lands on for a target
    /// aiming along `aim`: [`Front`](Self::Front) when the two point
    /// against each other (`direction · aim < 0`), else
    /// [`Rear`](Self::Rear); `None` when either is zero or not finite.
    #[must_use]
    pub fn of(direction: Vec2, aim: Vec2) -> Option<Side> {
        if direction == Vec2::ZERO
            || aim == Vec2::ZERO
            || !(direction.is_finite() && aim.is_finite())
        {
            return None;
        }
        Some(if direction.dot(aim) < 0.0 {
            Side::Front
        } else {
            Side::Rear
        })
    }

    /// The kit's tag for the side, which a hit's context carries:
    /// `side.front` or `side.rear`.
    #[must_use]
    pub fn tag(self) -> &'static str {
        match self {
            Side::Front => SIDE_FRONT,
            Side::Rear => SIDE_REAR,
        }
    }
}

impl DamageRequest {
    /// A request of one untyped part of `amount` on `target`, with no
    /// source, no types, no position, no knockback, no crit, not periodic,
    /// and no penetration or conversion of its own.
    ///
    /// # Panics
    ///
    /// When `amount` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn new(target: Entity, amount: f32) -> Self {
        assert!(
            amount.is_finite() && amount >= 0.0,
            "DamageRequest::new: amount must be finite and >= 0 (got {amount})"
        );
        Self::of(
            target,
            Parts::One(DamagePart {
                tags: Tags::default(),
                amount,
            }),
        )
    }

    /// A request of `parts` on `target`, in order, with the rest as
    /// [`new`](Self::new) leaves it; with no part it resolves
    /// [`Hit::Ignored`].
    #[must_use]
    pub fn from_parts(target: Entity, parts: impl IntoIterator<Item = DamagePart>) -> Self {
        Self::of(target, Parts::Many(parts.into_iter().collect()))
    }

    fn of(target: Entity, parts: Parts) -> Self {
        Self {
            target,
            parts,
            source: None,
            tags: Tags::default(),
            position: None,
            knockback: None,
            crit: Crit::Open,
            flags: HitFlags::default(),
            side: None,
            penetration: None,
            own: None,
            depth: 0,
        }
    }

    /// The request, `depth` chemistry reactions deep: its parts carry it,
    /// so a cycle of reactions through damage ends at the depth limit.
    pub(crate) fn with_depth(mut self, depth: u8) -> Self {
        self.depth = depth;
        self
    }

    /// The request, typed at a status's application: the source's
    /// conversion leaves its parts as they are.
    pub(crate) fn typed(mut self) -> Self {
        self.flags.set(HitFlags::TYPED, true);
        self
    }

    /// The boxed extras, made when the request has none.
    fn extras_mut(&mut self) -> &mut Extras {
        let own = match self.own.take() {
            Some(Own::Extras(extras)) => extras,
            Some(Own::Conversion(conversion)) => Box::new(Extras {
                conversion: Some(conversion),
                ..Extras::default()
            }),
            None => Box::default(),
        };
        let Own::Extras(extras) = self.own.insert(Own::Extras(own)) else {
            unreachable!("just inserted");
        };
        extras
    }

    fn extras(&self) -> Option<&Extras> {
        match &self.own {
            Some(Own::Extras(extras)) => Some(extras),
            _ => None,
        }
    }

    /// A request of `delivery`'s snapshot on `target` (§6.4 of the ARPG
    /// design): a part per snapshot part, of its damage type, reading its
    /// range's midpoint until the resolve draws it; the skill's context and
    /// the delivery's state tags; from the delivery's holder while it lives
    /// in `world`, else from no one. At the resolve the lines it left for
    /// the hit fold in (`against` on the target's tags, `with` on the state
    /// tags), one draw sets every part in its range, a second draws its
    /// crit under the snapshot's chance, and its parts meet the target's
    /// defences with their own penetration, its holder's tags at the use
    /// read for every `against`; it converts nothing further. A kit hit by
    /// an entity that carries a [`Delivery`] is one.
    #[must_use]
    pub fn from_delivery(world: &World, target: Entity, delivery: &Delivery) -> Self {
        let parts = delivery.parts.iter().map(|part| DamagePart {
            tags: Tags::new([part.kind.as_str()]),
            amount: f64::midpoint(f64::from(part.range[0]), f64::from(part.range[1])) as f32,
        });
        let mut request = Self::of(target, Parts::Many(parts.collect()));
        request.source = delivery.credit(world);
        let extras = request.extras_mut();
        extras.context = Tags::new(delivery.context().iter().map(String::as_str));
        extras.state = delivery.state().clone();
        extras.sheet = Some(delivery.clone());
        request
    }

    /// The snapshot it resolves from, for a delivery's hit.
    pub(crate) fn sheet(&self) -> Option<&Delivery> {
        self.extras().and_then(|extras| extras.sheet.as_ref())
    }

    /// The request, landing on `side`: a kit hit's, set at its strike.
    pub(crate) fn with_side(mut self, side: Option<Side>) -> Self {
        self.side = side;
        self
    }

    /// The request, its knockback landing on a target it is ignored for:
    /// contact damage's `push_always`.
    pub(crate) fn pushing_always(mut self) -> Self {
        self.flags.set(HitFlags::PUSH_ALWAYS, true);
        self
    }

    /// The request, from `source`: who gets the credit, whose
    /// [`Penetration`] and [`Conversion`] it takes after its own, and the
    /// cue events' other entity.
    #[must_use]
    pub fn with_source(mut self, source: Entity) -> Self {
        self.source = Some(source);
        self
    }

    /// The request, its parts without types of their own of the damage
    /// types `tags` (`damage.fire`), which a [`Resist`] reads; a converted
    /// share leaves them behind.
    #[must_use]
    pub fn with_tags(mut self, tags: Tags) -> Self {
        self.tags = tags;
        self
    }

    /// The request, with its own `penetration`, before its source's.
    #[must_use]
    pub fn with_penetration(mut self, penetration: Penetration) -> Self {
        self.penetration = Some(penetration);
        self
    }

    /// The request, with its own `conversion`, whose rules apply before its
    /// source's.
    #[must_use]
    pub fn with_conversion(mut self, conversion: Conversion) -> Self {
        match &mut self.own {
            Some(Own::Extras(extras)) => extras.conversion = Some(conversion),
            own => *own = Some(Own::Conversion(conversion)),
        }
        self
    }

    /// The request, in the context `tags`: a skill's or a game's tags, such
    /// as a blast's `hit.blast`, which the target's scoped resistances and
    /// `damage_taken` lines read. Only a request with a context, state tags
    /// or a direction boxes them.
    #[must_use]
    pub fn with_context(mut self, tags: Tags) -> Self {
        if !tags.is_empty() || self.extras().is_some() {
            self.extras_mut().context = tags;
        }
        self
    }

    /// The request, with the state tags `tags` (`delivery.chained`), known
    /// only at the hit, which the target's lines read as its context.
    #[must_use]
    pub fn with_state(mut self, tags: Tags) -> Self {
        if !tags.is_empty() || self.extras().is_some() {
            self.extras_mut().state = tags;
        }
        self
    }

    /// The request, travelling along `direction`: its side is judged
    /// against the target's [`Aim`] at the resolve, unless a kit hit's
    /// strike set it.
    ///
    /// # Panics
    ///
    /// When `direction` is not finite.
    #[must_use]
    #[track_caller]
    pub fn with_direction(mut self, direction: Vec2) -> Self {
        assert!(
            direction.is_finite(),
            "DamageRequest::with_direction: direction must be finite (got {direction})"
        );
        self.extras_mut().direction = Some(direction);
        self
    }

    /// The request, landing at `position`: where a `damaged` cue spawns or
    /// floats its label.
    ///
    /// # Panics
    ///
    /// When `position` is not finite.
    #[must_use]
    #[track_caller]
    pub fn with_position(mut self, position: Vec2) -> Self {
        assert!(
            position.is_finite(),
            "DamageRequest::with_position: position must be finite (got {position})"
        );
        self.position = Some(position);
        self
    }

    /// The request, pushing its target by `velocity` world pixels per
    /// second when it hurts or the shield absorbs it, through
    /// [`knock_back`](crate::knock_back).
    ///
    /// # Panics
    ///
    /// When `velocity` is not finite.
    #[must_use]
    #[track_caller]
    pub fn with_knockback(mut self, velocity: Vec2) -> Self {
        assert!(
            velocity.is_finite(),
            "DamageRequest::with_knockback: velocity must be finite (got {velocity})"
        );
        self.knockback = Some(velocity);
        self
    }

    /// The request, marked a critical hit when `crit`, its crit decided by
    /// the game: the kit carries the flag to [`Damaged`] and to each of its
    /// [`DamagedPart`]s, with no draw and no factor.
    #[must_use]
    pub fn with_crit(mut self, crit: bool) -> Self {
        self.crit = if crit { Crit::Yes } else { Crit::No };
        self
    }

    /// The request, its crit decided now by its builder from its source's
    /// entity-level [`Stats::CRIT_CHANCE`] and [`Stats::CRIT_MULTIPLIER`]
    /// (1.5 without one): one draw from the [`KitRng`] when the chance is
    /// above 0, every part times the multiplier on a crit, marked; no draw
    /// and no crit for a source without a chance, a request with no source,
    /// or a periodic one. The resolve draws no more. Set the source first.
    #[must_use]
    pub fn roll_crit(mut self, world: &mut World) -> Self {
        let odds = (!self.is_periodic())
            .then(|| crit_odds(world, self.source))
            .flatten();
        let crit = odds.is_some_and(|(chance, _)| draw_roll(world) < chance);
        if crit && let Some((_, multiplier)) = odds {
            let times = |part: &mut DamagePart| {
                part.amount = clamped(f64::from(part.amount) * f64::from(multiplier)).max(0.0);
            };
            match &mut self.parts {
                Parts::One(part) => times(part),
                Parts::Many(parts) => parts.iter_mut().for_each(times),
            }
        }
        self.crit = if crit { Crit::Yes } else { Crit::No };
        self
    }

    /// The request, periodic when `periodic`: an effect's payload, which
    /// passes the target's invulnerability window, starts none and starts
    /// no [`HitFlash`](crate::HitFlash); [`Damaged`] and [`DamagedPart`]
    /// carry the flag.
    #[must_use]
    pub fn with_periodic(mut self, periodic: bool) -> Self {
        self.flags.set(HitFlags::PERIODIC, periodic);
        self
    }

    /// The entity hit.
    #[must_use]
    pub fn target(&self) -> Entity {
        self.target
    }

    /// Its parts, in order.
    #[must_use]
    pub fn parts(&self) -> &[DamagePart] {
        self.parts.as_slice()
    }

    /// The parts' sum before resistances, clamped to `f32::MAX`.
    #[must_use]
    pub fn amount(&self) -> f32 {
        clamped(self.parts().iter().map(|part| f64::from(part.amount)).sum())
    }

    /// The entity it came from, when it has one.
    #[must_use]
    pub fn source(&self) -> Option<Entity> {
        self.source
    }

    /// The damage types its untyped parts take.
    #[must_use]
    pub fn tags(&self) -> &Tags {
        &self.tags
    }

    /// Its own penetration, when it has one.
    #[must_use]
    pub fn penetration(&self) -> Option<&Penetration> {
        self.penetration.as_ref()
    }

    /// Its own conversion, when it has one.
    #[must_use]
    pub fn conversion(&self) -> Option<&Conversion> {
        match &self.own {
            Some(Own::Conversion(conversion)) => Some(conversion),
            Some(Own::Extras(extras)) => extras.conversion.as_ref(),
            None => None,
        }
    }

    /// Its context tags; empty without.
    #[must_use]
    pub fn context(&self) -> &Tags {
        self.extras().map_or(&NO_TAGS, |extras| &extras.context)
    }

    /// Its state tags; empty without.
    #[must_use]
    pub fn state(&self) -> &Tags {
        self.extras().map_or(&NO_TAGS, |extras| &extras.state)
    }

    /// The direction a game gave it, when it has one.
    #[must_use]
    pub fn direction(&self) -> Option<Vec2> {
        self.extras().and_then(|extras| extras.direction)
    }

    /// The side a kit hit took at its strike; a game's direction is judged
    /// at the resolve.
    #[must_use]
    pub fn side(&self) -> Option<Side> {
        self.side
    }

    /// Where it landed, when it says.
    #[must_use]
    pub fn position(&self) -> Option<Vec2> {
        self.position
    }

    /// The push it gives, when it gives one.
    #[must_use]
    pub fn knockback(&self) -> Option<Vec2> {
        self.knockback
    }

    /// Whether it is a critical hit, as decided so far: a request the
    /// resolve decides reads false until then.
    #[must_use]
    pub fn is_crit(&self) -> bool {
        self.crit == Crit::Yes
    }

    /// Whether it is periodic: an effect's payload.
    #[must_use]
    pub fn is_periodic(&self) -> bool {
        self.flags.has(HitFlags::PERIODIC)
    }

    /// The request, lethal: it passes the conversion, the penetration, the
    /// resistances, the immunities and the window, empties the shield and
    /// takes all the health as one untyped part.
    pub(crate) fn lethal(mut self) -> Self {
        self.flags.set(HitFlags::LETHAL, true);
        self
    }

    /// Whether it is lethal ([`kill`](crate::kill)'s).
    pub(crate) fn is_lethal(&self) -> bool {
        self.flags.has(HitFlags::LETHAL)
    }

    /// Whether its knockback lands where the hit is ignored.
    fn pushes_always(&self) -> bool {
        self.flags.has(HitFlags::PUSH_ALWAYS)
    }

    /// Whether it was typed at a status's application.
    fn is_typed(&self) -> bool {
        self.flags.has(HitFlags::TYPED)
    }

    /// Resolves the request now, so the caller knows the outcome in the
    /// same step; see the type for the steps.
    pub fn apply(self, world: &mut World) -> Hit {
        merge_types(world);
        resolve(world, &self, &mut Vec::new(), &mut Vec::new())
    }

    /// Queues the request for [`DAMAGE_RESOLVE`], this step's when it has
    /// not run yet, else the next step's, so a request queued while the
    /// clock is paused waits for the next step. False, queueing nothing,
    /// when the World has no [`HealthPlugin`](crate::HealthPlugin) queue.
    pub fn queue(self, world: &mut World) -> bool {
        match world.get_resource_mut::<Requests>() {
            Some(requests) => {
                requests.damage.push(self);
                true
            }
            None => false,
        }
    }
}

/// `amount` clamped to `f32::MAX` and rounded to `f32`.
fn clamped(amount: f64) -> f32 {
    amount.min(f64::from(f32::MAX)) as f32
}

/// A heal to resolve: its target and amount, with the entity it came from.
/// Resolving one restores `min(amount, max − current)` to a live target
/// with [`Health`] that is not dead, sends [`Healed`] when it restored any
/// and records it in the trace; a heal never fills a [`Shield`]. [`heal`] is
/// the immediate form.
///
/// # Examples
///
/// ```
/// use tungsten_core::{Plugin, Schedule, World};
/// use tungsten_kit::{HealRequest, Health, HealthPlugin, damage};
///
/// let mut world = World::new();
/// let mut schedule = Schedule::new();
/// HealthPlugin.build(&mut schedule, &mut world);
///
/// let cleric = world.spawn();
/// let knight = world.spawn_with((Health::new(10.0),));
/// damage(&mut world, knight, 3.0);
/// assert_eq!(HealRequest::new(knight, 5.0).with_source(cleric).apply(&mut world), 3.0, "up to max");
/// assert_eq!(world.get::<Health>(knight).unwrap().current(), 10.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HealRequest {
    target: Entity,
    amount: f32,
    source: Option<Entity>,
}

impl HealRequest {
    /// A heal of `amount` on `target`, from no source.
    ///
    /// # Panics
    ///
    /// When `amount` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn new(target: Entity, amount: f32) -> Self {
        assert!(
            amount.is_finite() && amount >= 0.0,
            "HealRequest::new: amount must be finite and >= 0 (got {amount})"
        );
        Self {
            target,
            amount,
            source: None,
        }
    }

    /// The heal, from `source`: the `healed` cue event's other entity.
    #[must_use]
    pub fn with_source(mut self, source: Entity) -> Self {
        self.source = Some(source);
        self
    }

    /// The entity healed.
    #[must_use]
    pub fn target(&self) -> Entity {
        self.target
    }

    /// The amount asked for.
    #[must_use]
    pub fn amount(&self) -> f32 {
        self.amount
    }

    /// The entity it came from, when it has one.
    #[must_use]
    pub fn source(&self) -> Option<Entity> {
        self.source
    }

    /// Resolves the heal now; the health it restored.
    pub fn apply(self, world: &mut World) -> f32 {
        resolve_heal(world, &self)
    }

    /// Queues the heal for [`DAMAGE_RESOLVE`], after the step's damage
    /// requests; false, queueing nothing, without the queue.
    pub fn queue(self, world: &mut World) -> bool {
        match world.get_resource_mut::<Requests>() {
            Some(requests) => {
                requests.heals.push(self);
                true
            }
            None => false,
        }
    }
}

/// Heals `target` by `amount` at once: `HealRequest::new(target,
/// amount).apply(world)`, the health it restored. See [`HealRequest`].
///
/// # Panics
///
/// When `amount` is not finite and at least 0.
#[track_caller]
pub fn heal(world: &mut World, target: Entity, amount: f32) -> f32 {
    HealRequest::new(target, amount).apply(world)
}

/// A heal that restored health, sent once
/// [`HealthPlugin`](crate::HealthPlugin) has registered its queue: the cue
/// event `healed`. Build one with [`Healed::new`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct Healed {
    /// The entity healed.
    pub entity: Entity,
    /// The health restored.
    pub amount: f32,
    /// The entity the heal came from, when it had one.
    pub source: Option<Entity>,
}

impl Healed {
    /// An event with these fields and no source.
    #[must_use]
    pub fn new(entity: Entity, amount: f32) -> Self {
        Self {
            entity,
            amount,
            source: None,
        }
    }

    /// The event, from `source`.
    #[must_use]
    pub fn with_source(mut self, source: Entity) -> Self {
        self.source = Some(source);
        self
    }
}

/// The queued requests, a World resource the health plugin inserts.
#[derive(Debug, Default)]
pub(crate) struct Requests {
    damage: Vec<DamageRequest>,
    heals: Vec<HealRequest>,
}

/// The name a save holds the queued damage and heal requests under.
pub(crate) const DAMAGE_REQUESTS: &str = "damage_requests";

/// A damage part in a save: its damage types by name and its amount.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartSave {
    #[serde(default, skip_serializing_if = "Tags::is_empty")]
    tags: Tags,
    amount: f32,
}

/// A [`DamageRequest`] in a save, its target and source by reference.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)] // A save's flags, each a field of its JSON as before.
struct DamageSave {
    parts: Vec<PartSave>,
    #[serde(default, skip_serializing_if = "Tags::is_empty")]
    tags: Tags,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    position: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    knockback: Option<[f32; 2]>,
    #[serde(default)]
    crit: bool,
    /// Its crit decided with no crit: a request the resolve decides writes
    /// none.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    crit_decided: bool,
    #[serde(default)]
    periodic: bool,
    #[serde(default)]
    push_always: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    penetration: Option<Penetration>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    conversion: Option<Conversion>,
    #[serde(default, skip_serializing_if = "Tags::is_empty")]
    context: Tags,
    #[serde(default, skip_serializing_if = "Tags::is_empty")]
    state: Tags,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    direction: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    side: Option<Side>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sheet: Option<Delivery>,
    #[serde(default, skip_serializing_if = "is_zero")]
    depth: u8,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    typed: bool,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(value: &u8) -> bool {
    *value == 0
}

impl From<&DamageRequest> for DamageSave {
    fn from(request: &DamageRequest) -> Self {
        Self {
            parts: request
                .parts
                .as_slice()
                .iter()
                .map(|part| PartSave {
                    tags: part.tags.clone(),
                    amount: part.amount,
                })
                .collect(),
            tags: request.tags.clone(),
            position: request.position.map(|at| at.to_array()),
            knockback: request.knockback.map(|push| push.to_array()),
            crit: request.crit == Crit::Yes,
            crit_decided: request.crit == Crit::No,
            periodic: request.is_periodic(),
            push_always: request.pushes_always(),
            penetration: request.penetration.clone(),
            conversion: request.conversion().cloned(),
            context: request.context().clone(),
            state: request.state().clone(),
            direction: request.direction().map(|at| at.to_array()),
            side: request.side,
            sheet: request.sheet().cloned(),
            depth: request.depth,
            typed: request.is_typed(),
        }
    }
}

impl From<DamageSave> for DamageRequest {
    /// A part's amount that is not finite and at least 0 reads as 0.
    fn from(saved: DamageSave) -> Self {
        let mut parts: Vec<DamagePart> = saved
            .parts
            .into_iter()
            .map(|part| DamagePart {
                tags: part.tags,
                amount: if part.amount.is_finite() {
                    part.amount.max(0.0)
                } else {
                    0.0
                },
            })
            .collect();
        let parts = if parts.len() == 1 {
            Parts::One(parts.remove(0))
        } else {
            Parts::Many(parts)
        };
        let mut request = Self {
            target: Entity::PLACEHOLDER,
            parts,
            source: None,
            tags: saved.tags,
            position: saved.position.map(Vec2::from_array),
            knockback: saved.knockback.map(Vec2::from_array),
            crit: match (saved.crit, saved.crit_decided) {
                (true, _) => Crit::Yes,
                (false, true) => Crit::No,
                (false, false) => Crit::Open,
            },
            flags: HitFlags::default(),
            side: saved.side,
            penetration: saved.penetration,
            own: None,
            depth: saved.depth,
        };
        request.flags.set(HitFlags::PERIODIC, saved.periodic);
        request.flags.set(HitFlags::PUSH_ALWAYS, saved.push_always);
        request.flags.set(HitFlags::TYPED, saved.typed);
        if let Some(conversion) = saved.conversion {
            request = request.with_conversion(conversion);
        }
        request = request.with_context(saved.context).with_state(saved.state);
        if let Some(direction) = saved.direction.map(Vec2::from_array)
            && direction.is_finite()
        {
            request = request.with_direction(direction);
        }
        if let Some(sheet) = saved.sheet {
            request.extras_mut().sheet = Some(sheet);
        }
        request
    }
}

/// A [`HealRequest`] in a save: its amount.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HealSave {
    amount: f32,
}

/// The queued requests in a save.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestsSave {
    #[serde(default)]
    damage: Vec<DamageSave>,
    #[serde(default)]
    heals: Vec<HealSave>,
}

impl From<&Requests> for RequestsSave {
    fn from(requests: &Requests) -> Self {
        Self {
            damage: requests.damage.iter().map(Into::into).collect(),
            heals: requests
                .heals
                .iter()
                .map(|heal| HealSave {
                    amount: heal.amount,
                })
                .collect(),
        }
    }
}

impl From<RequestsSave> for Requests {
    fn from(saved: RequestsSave) -> Self {
        Self {
            damage: saved.damage.into_iter().map(Into::into).collect(),
            heals: saved
                .heals
                .into_iter()
                .map(|heal| HealRequest {
                    target: Entity::PLACEHOLDER,
                    amount: if heal.amount.is_finite() {
                        heal.amount.max(0.0)
                    } else {
                        0.0
                    },
                    source: None,
                })
                .collect(),
        }
    }
}

/// Each queued request's target and source, by key.
fn request_refs(requests: &Requests) -> impl Iterator<Item = (String, Entity)> + '_ {
    let damage = requests
        .damage
        .iter()
        .enumerate()
        .flat_map(|(index, request)| {
            std::iter::once((format!("damage.{index}.target"), request.target)).chain(
                request
                    .source
                    .map(|source| (format!("damage.{index}.source"), source)),
            )
        });
    let heals = requests
        .heals
        .iter()
        .enumerate()
        .flat_map(|(index, request)| {
            std::iter::once((format!("heal.{index}.target"), request.target)).chain(
                request
                    .source
                    .map(|source| (format!("heal.{index}.source"), source)),
            )
        });
    damage.chain(heals)
}

/// The queued damage and heal requests, saved so a request queued after a
/// step's resolve resolves once after a load; each target and source by
/// save ID.
pub(crate) fn requests_saved() -> SaveOptions<Requests> {
    SaveOptions::<Requests>::form::<RequestsSave>().refs(RefCodec {
        save: |requests, out: &mut RefWriter<'_>| {
            for (key, entity) in request_refs(requests) {
                out.put(&key, entity);
            }
        },
        load: |requests, refs: &RefReader<'_>| {
            for (index, request) in requests.damage.iter_mut().enumerate() {
                if let Some(target) = refs.first(&format!("damage.{index}.target")) {
                    request.target = target;
                }
                if let Some(source) = refs.first(&format!("damage.{index}.source")) {
                    request.source = Some(source);
                }
            }
            for (index, request) in requests.heals.iter_mut().enumerate() {
                if let Some(target) = refs.first(&format!("heal.{index}.target")) {
                    request.target = target;
                }
                if let Some(source) = refs.first(&format!("heal.{index}.source")) {
                    request.source = Some(source);
                }
            }
        },
        keys: |requests| request_refs(requests).map(|(key, _)| key).collect(),
    })
}

#[cfg(test)]
impl Requests {
    /// The queued damage requests, in queue order.
    pub(crate) fn damage(&self) -> &[DamageRequest] {
        &self.damage
    }
}

#[cfg(test)]
impl DamageRequest {
    /// Whether it boxed its extras.
    pub(crate) fn has_extras(&self) -> bool {
        self.extras().is_some()
    }
}

/// [`DAMAGE_RESOLVE`]'s state between steps: the drained lists and a
/// resolution's landed and immune parts, kept so a step allocates nothing
/// once warm.
#[derive(Default)]
pub(crate) struct Resolver {
    damage: Vec<DamageRequest>,
    heals: Vec<HealRequest>,
    landed: Vec<Landed>,
    immune: Vec<Landed>,
}

impl Resolver {
    /// Merges the type table once, then resolves the queued damage
    /// requests, stable-sorted by target index and then source index with
    /// none first, then the heals the same way.
    pub(crate) fn run(&mut self, world: &mut World) {
        let Some(requests) = world.get_resource_mut::<Requests>() else {
            return;
        };
        if requests.damage.is_empty() && requests.heals.is_empty() {
            return;
        }
        std::mem::swap(&mut self.damage, &mut requests.damage);
        std::mem::swap(&mut self.heals, &mut requests.heals);
        if !self.damage.is_empty() {
            merge_types(world);
        }
        let order = order_of(world);
        self.damage.sort_by_key(|request| {
            (
                order.key(request.target),
                request.source.map(|source| order.key(source)),
            )
        });
        for request in self.damage.drain(..) {
            resolve(world, &request, &mut self.landed, &mut self.immune);
        }
        let order = order_of(world);
        self.heals.sort_by_key(|request| {
            (
                order.key(request.target),
                request.source.map(|source| order.key(source)),
            )
        });
        for request in self.heals.drain(..) {
            resolve_heal(world, &request);
        }
    }
}

/// An ignored request that pushes always ([`DamageRequest::pushing_always`])
/// still lands its knockback on a target alive and not being despawned by
/// the kit; `knock_back` spares the dead.
fn push_ignored(world: &mut World, request: &DamageRequest) {
    if request.pushes_always()
        && let Some(velocity) = request.knockback
        && world.is_alive(request.target)
        && !world.has::<Despawning>(request.target)
    {
        knock_back(world, request.target, velocity);
    }
}

/// Whether `target` takes a request: alive, not being despawned by the kit,
/// with [`Health`] that is neither dead nor, unless the request is
/// `periodic`, inside its window.
fn takes_hits(world: &World, target: Entity, periodic: bool) -> bool {
    world.is_alive(target)
        && !world.has::<Despawning>(target)
        && world
            .get::<Health>(target)
            .is_some_and(|health| !health.is_dead() && (periodic || !health.is_invulnerable()))
}

/// One part after the target's resistances: its amount and its kinds.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Landed {
    amount: f32,
    kinds: DamageKinds,
    /// Its amount before the factors and the crit, times the hit's status
    /// effect: a damage-over-time rate's base.
    dot_base: f32,
}

/// A part between the conversion and the resistances: its types and its
/// amount in `f64`.
type Working<'a> = (&'a Tags, f64);

/// What the target's defences read of a request: its resistances, the
/// penetration of the hit and of its source, the type table, the target's
/// stats for `damage_taken`, and the hit's context.
struct Defences<'a> {
    resist: Option<&'a Resist>,
    own: Option<&'a Penetration>,
    lent: Option<&'a Penetration>,
    table: Option<&'a DamageTypeTable>,
    /// The target's stats, when they hold `damage_taken`.
    taken: Option<&'a Stats>,
    hit: HitContext<'a>,
}

/// A hit's context as the target's lines read it: the request's context
/// and state tags, its track and side, the target's tags and the
/// attacker's.
struct HitContext<'a> {
    context: &'a Tags,
    state: &'a Tags,
    track: &'static str,
    side: Option<&'static str>,
    holder: Option<&'a Tags>,
    attacker: Option<&'a Tags>,
    periodic: bool,
}

/// What one part comes to after the target's defences.
#[derive(Clone, Copy)]
enum Landing {
    /// It lands, its amount after the resistances.
    Landed(Landed),
    /// A factor of 0 dropped it: its amount before the resistances.
    Immune(Landed),
    /// It came to 0.
    Dropped,
}

impl Defences<'_> {
    /// Whether no line of the target's could read anything of the hit's
    /// context: no `damage_taken`, and no floor or scoped resistance line.
    fn plain(&self) -> bool {
        self.taken.is_none() && self.resist.is_none_or(|resist| !resist.reads_context())
    }

    /// Step 3 for one part: the request's points and its source's, none on
    /// a periodic hit.
    fn points(&self, tags: &Tags) -> f32 {
        if self.hit.periodic {
            0.0
        } else {
            self.own.map_or(0.0, |own| own.sum(tags)) + self.lent.map_or(0.0, |lent| lent.sum(tags))
        }
    }

    /// Steps 3 and 4 for one part: its points, then its factors and the
    /// target's `damage_taken`, read in the hit's context.
    fn land(&self, tags: &Tags, amount: f64) -> Landing {
        self.land_with(tags, &[], amount, self.points(tags))
    }

    /// The sets the target's lines read a part of `tags` and `history` in:
    /// the hit's context, track, state tags and side, the part's types and
    /// its history.
    fn sets<'s>(&'s self, tags: &'s Tags, history: &'s [String]) -> [TagSet<'s>; 6] {
        let hit = &self.hit;
        [
            TagSet::Tags(hit.context),
            TagSet::Name(hit.track),
            TagSet::Tags(hit.state),
            hit.side.map_or(TagSet::Names(&[]), TagSet::Name),
            TagSet::Tags(tags),
            TagSet::Names(history),
        ]
    }

    /// Step 4 for one part as its record keeps it: the resistance's factor
    /// in the hit's context, the floor of the part's types, the `points` and
    /// the product they make, the target's `damage_taken`, and what
    /// `landing` left.
    fn note(
        &self,
        part: &mut HitPartRecord,
        tags: &Tags,
        history: &[String],
        points: f32,
        landing: Landing,
    ) {
        let sets = self.sets(tags, history);
        let read = Read::new(&sets)
            .holder(self.hit.holder)
            .against(self.hit.attacker);
        let points = points.min(1.0);
        let plain = self.plain();
        part.resist = self.resist.map(|resist| {
            let (factor, product) = if plain {
                (resist.factor(tags), resist.product_with(tags, points))
            } else {
                (
                    resist.factor_in(tags, &read),
                    resist.product_in(tags, points, &read),
                )
            };
            // A floor on a type or its ancestors, as the factors match.
            let floor = tags.iter().find_map(|tag| {
                std::iter::successors(Some(tag), |tag| tag.rsplit_once('.').map(|(up, _)| up))
                    .find_map(|tag| resist.floor(tag))
            });
            ResistRecord {
                factor,
                floor,
                pen: points,
                product: product.unwrap_or(0.0),
            }
        });
        part.taken = if plain {
            1.0
        } else {
            self.taken
                .and_then(|stats| stats.read(Stats::DAMAGE_TAKEN, &read))
                .map_or(1.0, f64::from)
        };
        (part.after, part.immune) = match landing {
            Landing::Landed(landed) => (landed.amount, false),
            Landing::Immune(_) => (0.0, true),
            Landing::Dropped => (0.0, false),
        };
    }

    /// A request's part without a sheet as its record keeps it: its types,
    /// its amount `before` the defences, the crit's factor `crit` in it, and
    /// step 4.
    fn noted(&self, tags: &Tags, before: f64, crit: f64, landing: Landing) -> HitPartRecord {
        let mut part = HitPartRecord {
            tags: tags.iter().map(str::to_owned).collect(),
            history: Vec::new(),
            origins: Vec::new(),
            base: if crit > 0.0 { before / crit } else { before },
            lines: Vec::new(),
            pct: 0.0,
            mul: 1.0,
            factor: 1.0,
            crit,
            before,
            resist: None,
            taken: 1.0,
            after: 0.0,
            immune: false,
        };
        self.note(&mut part, tags, &[], self.points(tags), landing);
        part
    }

    /// Step 4 for one part of `tags` and `history`, with `points` of
    /// penetration: its factors and the target's `damage_taken`, read in
    /// the hit's context.
    fn land_with(&self, tags: &Tags, history: &[String], amount: f64, points: f32) -> Landing {
        let kinds = || {
            self.table
                .map_or(DamageKinds::NONE, |table| table.mask(tags))
        };
        let hit = &self.hit;
        let amount = if self.plain() {
            match self.resist {
                None => clamped(amount),
                Some(resist) => {
                    let Some(product) = resist.product_with(tags, points.min(1.0)) else {
                        return Landing::Immune(Landed {
                            amount: clamped(amount),
                            kinds: kinds(),
                            dot_base: 0.0,
                        });
                    };
                    clamped(amount * product)
                }
            }
        } else {
            let sets = self.sets(tags, history);
            let read = Read::new(&sets).holder(hit.holder).against(hit.attacker);
            let product = match self.resist {
                None => Some(1.0),
                Some(resist) => resist.product_in(tags, points.min(1.0), &read),
            };
            let Some(product) = product else {
                return Landing::Immune(Landed {
                    amount: clamped(amount),
                    kinds: kinds(),
                    dot_base: 0.0,
                });
            };
            let taken = self
                .taken
                .and_then(|stats| stats.read(Stats::DAMAGE_TAKEN, &read))
                .map_or(1.0, f64::from);
            clamped(amount * product * taken)
        };
        if amount == 0.0 {
            return Landing::Dropped;
        }
        Landing::Landed(Landed {
            amount,
            kinds: kinds(),
            dot_base: 0.0,
        })
    }
}

/// A hit's status effect: its sheet's, snapshot at the use, else its
/// source's [`Stats::STATUS_EFFECT`] at the resolve, else 1.
fn hit_status_effect(world: &World, request: &DamageRequest) -> f32 {
    match request.sheet() {
        Some(sheet) => sheet.status_effect(),
        None => request
            .source
            .and_then(|source| world.get::<Stats>(source))
            .and_then(|stats| stats.get(Stats::STATUS_EFFECT))
            .unwrap_or(1.0),
    }
}

/// What a request's parts are multiplied by before the conversion: its live
/// source's [`Stats::DAMAGE_DEALT`] read in the hit's context, at least 0;
/// 1 for a periodic request and a source gone or without the stat, which
/// reads nothing more.
fn dealt(world: &World, request: &DamageRequest, side: Option<Side>) -> f64 {
    if request.is_periodic() {
        return 1.0;
    }
    let Some(source) = request.source.filter(|&source| world.is_alive(source)) else {
        return 1.0;
    };
    let Some(stats) = world
        .get::<Stats>(source)
        .filter(|stats| stats.get(Stats::DAMAGE_DEALT).is_some())
    else {
        return 1.0;
    };
    let sets = [
        TagSet::Tags(request.context()),
        TagSet::Name(TRACK_HIT),
        TagSet::Tags(request.state()),
        side.map_or(TagSet::Names(&[]), |side| TagSet::Name(side.tag())),
    ];
    let read = Read::new(&sets)
        .holder(world.get::<Tags>(source))
        .against(world.get::<Tags>(request.target));
    stats
        .read(Stats::DAMAGE_DEALT, &read)
        .map_or(1.0, |value| f64::from(value.max(0.0)))
}

/// A game's hit preparation, registered by name with [`add_hit_prep`]: it
/// reads the World and the hit a [`Blast`](crate::Blast) or an impact
/// builds, its target ([`DamageRequest::target`]) and point
/// ([`DamageRequest::position`]) among it, and changes the hit, such as a
/// penetration by the side of the target its point lies on.
pub type HitPrep = fn(&World, &mut DamageRequest);

/// The hit preparations a game registered, by name, and the names a Rust
/// blast gave that none holds, each warned of once: a World resource
/// [`add_hit_prep`] inserts.
#[derive(Debug, Default)]
pub(crate) struct HitPreps {
    preps: Vec<(String, HitPrep)>,
    warned: Vec<String>,
}

/// Registers `prep` as the hit preparation `name`, replacing one of that
/// name, for the blasts and impacts whose `prepare` names it: each runs it
/// on each hit it builds, after the hit's own fields and before its crit's
/// roll. Register it before the data that names it loads: the load refuses
/// a name no preparation holds.
///
/// # Panics
///
/// When `name` is not `[a-z][a-z0-9_]*`.
///
/// # Examples
///
/// ```
/// use tungsten_core::World;
/// use tungsten_kit::{DamageRequest, Penetration, add_hit_prep};
///
/// // Every hit it prepares passes physical resistance whole.
/// fn armour(_: &World, hit: &mut DamageRequest) {
///     *hit = hit
///         .clone()
///         .with_penetration(Penetration::new().with_points("physical", 1.0));
/// }
///
/// let mut world = World::new();
/// add_hit_prep(&mut world, "armour", armour);
/// ```
#[track_caller]
pub fn add_hit_prep(world: &mut World, name: impl Into<String>, prep: HitPrep) {
    let name = name.into();
    assert!(
        crate::foundation::stats::valid_stat(&name),
        "add_hit_prep: `{name}` is not a preparation name (`[a-z][a-z0-9_]*`)"
    );
    if !world.has_resource::<HitPreps>() {
        world.insert_resource(HitPreps::default());
    }
    let preps = world.resource_mut::<HitPreps>();
    preps.preps.retain(|(known, _)| *known != name);
    preps.preps.push((name, prep));
}

/// The hit preparation `name`, when a game registered it.
pub(crate) fn hit_prep(world: &World, name: &str) -> Option<HitPrep> {
    world.get_resource::<HitPreps>().and_then(|preps| {
        preps
            .preps
            .iter()
            .find(|(known, _)| known == name)
            .map(|&(_, prep)| prep)
    })
}

/// The load's check of a `prepare` at `field`: a name some preparation
/// holds.
pub(crate) fn check_hit_prep(world: &World, field: &str, name: &str) -> Result<(), MissingAsset> {
    match hit_prep(world, name) {
        Some(_) => Ok(()),
        None => Err(MissingAsset::new(field, "hit preparation", name)),
    }
}

/// Warns, once a World for each name, that `what` was refused for naming
/// `name`, which no preparation holds.
pub(crate) fn refuse_hit_prep(world: &mut World, what: &str, name: &str) {
    if !world.has_resource::<HitPreps>() {
        world.insert_resource(HitPreps::default());
    }
    let preps = world.resource_mut::<HitPreps>();
    if preps.warned.iter().any(|warned| warned == name) {
        return;
    }
    preps.warned.push(name.to_owned());
    log::warn!(
        "{what} refused: no hit preparation `{name}` is registered (add_hit_prep); later ones go unwarned"
    );
}

/// Puts one part's landing in its list, with its damage-over-time base.
fn record(landing: Landing, dot_base: f64, landed: &mut Vec<Landed>, immune: &mut Vec<Landed>) {
    let dot_base = dot_base.clamp(0.0, f64::from(f32::MAX)) as f32;
    match landing {
        Landing::Landed(part) => landed.push(Landed { dot_base, ..part }),
        Landing::Immune(part) => immune.push(Landed { dot_base, ..part }),
        Landing::Dropped => {}
    }
}

/// Steps 1 to 4 of `request`'s resolution: its parts after the
/// conversion, the penetration and the target's resistances, in part
/// order, into `landed`, and those an immunity dropped into `immune`;
/// `landed` is empty when every part dropped. With `notes`, each part's
/// defences go in its record.
fn land_parts<'a>(
    world: &'a World,
    request: &'a DamageRequest,
    crit: f64,
    drawn: &[(f64, f32, f64)],
    landed: &mut Vec<Landed>,
    immune: &mut Vec<Landed>,
    mut notes: Option<&mut HitRecord>,
) {
    landed.clear();
    immune.clear();
    let target = request.target;
    let status_effect = f64::from(hit_status_effect(world, request));
    let side = request.side.or_else(|| {
        let direction = request.direction()?;
        let aim = world.get::<Aim>(target)?.direction()?;
        Side::of(direction, aim)
    });
    let dealt = dealt(world, request, side);
    let defences = Defences {
        resist: world.get::<Resist>(target),
        own: request.penetration.as_ref(),
        lent: request
            .source
            .and_then(|source| world.get::<Penetration>(source)),
        table: world.get_resource::<DamageTypeTable>(),
        taken: world
            .get::<Stats>(target)
            .filter(|stats| stats.get(Stats::DAMAGE_TAKEN).is_some()),
        hit: HitContext {
            context: request.context(),
            state: request.state(),
            track: if request.is_periodic() {
                TRACK_DOT
            } else {
                TRACK_HIT
            },
            side: side.map(Side::tag),
            holder: world.get::<Tags>(target),
            attacker: match request.sheet() {
                Some(sheet) => Some(sheet.holder_tags()),
                None => request.source.and_then(|source| world.get::<Tags>(source)),
            },
            periodic: request.is_periodic(),
        },
    };
    // A delivery's hit: its drawn amounts and own points, converted at use.
    if let Some(sheet) = request.sheet() {
        for (at, (part, &(amount, points, dot_base))) in sheet.parts.iter().zip(drawn).enumerate() {
            let amount = amount * dealt;
            if amount > 0.0 {
                let tags = Tags::new([part.kind.as_str()]);
                let amount = amount.min(f64::from(f32::MAX));
                let landing = defences.land_with(&tags, &part.history, amount, points);
                if let Some(noted) = notes
                    .as_deref_mut()
                    .and_then(|notes| notes.parts.get_mut(at))
                {
                    defences.note(noted, &tags, &part.history, points, landing);
                }
                record(landing, dot_base * dealt, landed, immune);
            }
        }
        return;
    }
    let own_rules = request.conversion().map_or(&[][..], Conversion::rules);
    // A status's typed ticks skip the source's live conversion.
    let lent_rules = request
        .source
        .filter(|_| !request.is_typed())
        .and_then(|source| world.get::<Conversion>(source))
        .map_or(&[][..], Conversion::rules);
    let parts = request.parts.as_slice();
    let typed = |part: &'a DamagePart| -> &'a Tags {
        if part.tags.is_empty() {
            &request.tags
        } else {
            &part.tags
        }
    };
    if let [part] = parts
        && own_rules.is_empty()
        && lent_rules.is_empty()
    {
        let dealt_amount = f64::from(part.amount) * dealt;
        if dealt_amount != 0.0 {
            let amount = (dealt_amount * crit).min(f64::from(f32::MAX));
            let landing = defences.land(typed(part), amount);
            if let Some(notes) = notes {
                notes
                    .parts
                    .push(defences.noted(typed(part), amount, crit, landing));
            }
            record(landing, dealt_amount * status_effect, landed, immune);
        }
        return;
    }
    let mut working: Vec<Working<'_>> = Vec::with_capacity(parts.len() + 1);
    for part in parts.iter().filter(|part| part.amount != 0.0) {
        convert(
            typed(part),
            f64::from(part.amount) * dealt * crit,
            own_rules.iter().chain(lent_rules),
            &mut working,
        );
    }
    for &(tags, amount) in &working {
        let amount = amount.min(f64::from(f32::MAX));
        let landing = defences.land(tags, amount);
        if let Some(notes) = notes.as_deref_mut() {
            notes
                .parts
                .push(defences.noted(tags, amount, crit, landing));
        }
        // Its amount before the crit: the conversion is linear.
        record(landing, amount / crit * status_effect, landed, immune);
    }
}

/// Step 2 for one part of `tags` and `amount`: the matching `rules` take
/// their shares, normalized when their `pct`s sum past 1, each a part of
/// its rule's type alone; the remainder, then the shares in rule order, go
/// into `working`, each merged into a part of equal types already there.
fn convert<'a>(
    tags: &'a Tags,
    amount: f64,
    rules: impl Iterator<Item = &'a ConversionRule> + Clone,
    working: &mut Vec<Working<'a>>,
) {
    let matching = rules.filter(|rule| tags.has(rule.from()));
    let sum: f64 = matching.clone().map(|rule| f64::from(rule.pct())).sum();
    if sum < 1.0 {
        merge_part(working, tags, amount * (1.0 - sum));
    }
    for rule in matching {
        let pct = f64::from(rule.pct());
        let share = if sum > 1.0 {
            amount * pct / sum
        } else {
            amount * pct
        };
        merge_part(working, rule.to(), share);
    }
}

/// Adds `amount` of `tags` to the part of equal types in `working`, or
/// appends it; an amount of 0 is no part.
fn merge_part<'a>(working: &mut Vec<Working<'a>>, tags: &'a Tags, amount: f64) {
    if amount <= 0.0 {
        return;
    }
    match working
        .iter_mut()
        .find(|(known, _)| std::ptr::eq(*known, tags) || *known == tags)
    {
        Some((_, sum)) => *sum += amount,
        None => working.push((tags, amount)),
    }
}

/// Step 4's [`ImmunePart`]s: one per part of `request` an immunity dropped,
/// in part order, once [`HealthPlugin`](crate::HealthPlugin) has
/// registered the queue, with the hit's `crit`.
fn send_immune(world: &mut World, request: &DamageRequest, immune: &[Landed], crit: bool) {
    if !world.has_event::<ImmunePart>() {
        return;
    }
    let queue = world.resource_mut::<EventQueue<ImmunePart>>();
    for part in immune {
        let mut event = ImmunePart::new(request.target, part.amount)
            .with_kinds(part.kinds)
            .with_periodic(request.is_periodic())
            .with_crit(crit);
        if let Some(source) = request.source {
            event = event.with_source(source);
        }
        if let Some(position) = request.position {
            event = event.with_position(position);
        }
        queue.send(event);
    }
}

/// [`DamageRequest`]'s resolution, its landed parts gathered in `landed`
/// and those an immunity dropped in `immune`.
fn resolve(
    world: &mut World,
    request: &DamageRequest,
    landed: &mut Vec<Landed>,
    immune: &mut Vec<Landed>,
) -> Hit {
    let target = request.target;
    let mut crit = request.crit == Crit::Yes;
    let mut factor = 1.0;
    let mut drawn = Vec::new();
    let mut notes = world
        .has_resource::<KitExplain>()
        .then(|| HitRecord::new(target, request.source, request.is_periodic()));
    if let Some(sheet) = request.sheet() {
        // A delivery's hit: the range draw, then the crit draw.
        if !world.has_resource::<KitRng>() {
            world.insert_resource(KitRng::default());
        }
        let rng = world.resource_mut::<KitRng>();
        let (u, roll) = (rng.unit(), rng.unit());
        crit = sheet.amounts(
            world.get::<Tags>(target),
            u,
            roll,
            &mut drawn,
            notes.as_mut(),
        );
    } else if request.crit == Crit::Open
        && !request.is_periodic()
        && let Some((chance, multiplier)) = crit_odds(world, request.source)
    {
        let roll = draw_roll(world);
        if roll < chance {
            crit = true;
            factor = f64::from(multiplier);
        }
        if let Some(notes) = &mut notes {
            notes.crit = CritRecord::Drawn {
                chance,
                roll,
                multiplier,
            };
        }
    } else if request.crit != Crit::Open
        && let Some(notes) = &mut notes
    {
        notes.crit = CritRecord::Decided(crit);
    }
    let lethal = request.is_lethal();
    if request.parts().iter().all(|part| part.amount == 0.0)
        || !takes_hits(world, target, request.is_periodic() || lethal)
    {
        push_ignored(world, request);
        return Hit::Ignored;
    }
    let status_effect = hit_status_effect(world, request);
    if lethal {
        // All the shield and the health as one untyped part, past every
        // defence.
        landed.clear();
        immune.clear();
        let amount = world.get::<Shield>(target).map_or(0.0, Shield::current)
            + world.get::<Health>(target).map_or(0.0, Health::current);
        landed.push(Landed {
            amount,
            kinds: DamageKinds::NONE,
            dot_base: amount,
        });
        if let Some(notes) = &mut notes {
            notes.lethal = true;
        }
    } else {
        land_parts(
            world,
            request,
            factor,
            &drawn,
            landed,
            immune,
            notes.as_mut(),
        );
    }
    if !immune.is_empty() {
        send_immune(world, request, immune, crit);
    }
    if landed.is_empty() {
        keep(world, notes);
        push_ignored(world, request);
        return Hit::Ignored;
    }
    let amount = clamped(landed.iter().map(|part| f64::from(part.amount)).sum());
    let kinds = landed
        .iter()
        .fold(DamageKinds::NONE, |kinds, part| kinds.union(part.kinds));
    let absorbed = world
        .get_mut::<Shield>(target)
        .map_or(0.0, |shield| shield.take(amount));
    let rest = amount - absorbed;
    let killed = world
        .get_mut::<Health>(target)
        .unwrap()
        .hurt(rest, request.is_periodic());
    if killed && world.get::<Health>(target).is_some_and(Health::disarms) {
        disarm(world, target);
    }
    if let Some(mut notes) = notes {
        notes.amount = amount;
        notes.killed = killed;
        keep(world, Some(notes));
    }
    if !request.is_periodic() {
        start_flash_as(world, target, absorbed > 0.0 && rest <= 0.0);
    }
    if world.has_event::<DamagedPart>() {
        let queue = world.resource_mut::<EventQueue<DamagedPart>>();
        for part in landed.iter() {
            let mut event = DamagedPart::new(target, part.amount)
                .with_kinds(part.kinds)
                .with_periodic(request.is_periodic())
                .with_crit(crit)
                .with_dot_base(part.dot_base)
                .with_status_effect(status_effect)
                .with_depth(request.depth);
            if let Some(source) = request.source {
                event = event.with_source(source);
            }
            if let Some(position) = request.position {
                event = event.with_position(position);
            }
            queue.send(event);
        }
    }
    if world.has_event::<Damaged>() {
        let mut event = Damaged::new(target, amount, killed)
            .with_crit(crit)
            .with_periodic(request.is_periodic())
            .with_kinds(kinds)
            .with_absorbed(absorbed);
        if let Some(source) = request.source {
            event = event.with_source(source);
        }
        if let Some(position) = request.position {
            event = event.with_position(position);
        }
        world.resource_mut::<EventQueue<Damaged>>().send(event);
    }
    for part in landed.iter() {
        trace(world, target, || TraceKind::Event {
            name: "damaged_part",
            other: request.source,
            amount: Some(part.amount),
        });
    }
    trace(world, target, || TraceKind::Event {
        name: "damaged",
        other: request.source,
        amount: Some(amount),
    });
    if killed {
        trace(world, target, || TraceKind::Event {
            name: "killed",
            other: request.source,
            amount: Some(amount),
        });
        return Hit::Killed;
    }
    if let Some(velocity) = request.knockback {
        knock_back(world, target, velocity);
    }
    if rest > 0.0 { Hit::Hurt } else { Hit::Absorbed }
}

/// Puts a hit's record in the world's [`KitExplain`], while one is kept.
fn keep(world: &World, notes: Option<HitRecord>) {
    if let Some(notes) = notes
        && let Some(explain) = world.get_resource::<KitExplain>()
    {
        explain.push(ExplainRecord::Hit(notes));
    }
}

/// [`HealRequest`]'s resolution: the health restored.
fn resolve_heal(world: &mut World, request: &HealRequest) -> f32 {
    let target = request.target;
    if request.amount == 0.0 || !world.is_alive(target) || world.has::<Despawning>(target) {
        return 0.0;
    }
    let Some(health) = world.get_mut::<Health>(target) else {
        return 0.0;
    };
    let restored = health.restore(request.amount);
    if restored > 0.0 {
        if world.has_event::<Healed>() {
            let mut event = Healed::new(target, restored);
            if let Some(source) = request.source {
                event = event.with_source(source);
            }
            world.resource_mut::<EventQueue<Healed>>().send(event);
        }
        trace(world, target, || TraceKind::Event {
            name: "healed",
            other: request.source,
            amount: Some(restored),
        });
    }
    restored
}

#[cfg(test)]
#[path = "../tests/pipeline.rs"]
mod tests;
