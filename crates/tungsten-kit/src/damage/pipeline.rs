//! The damage pipeline: requests of typed parts that carry a source, a
//! position, a knockback, a crit flag and a periodic flag, resolved through
//! conversion, penetration and resistances per part, then once through a
//! shield and health, at once or once a step in sorted order; heals the
//! same way.

use glam::Vec2;
use tungsten_core::{Entity, EventQueue, World};

use crate::damage::defense::{Resist, Shield};
use crate::damage::health::{Damaged, DamagedPart, Health, Hit, start_flash};
use crate::damage::kinds::{
    Conversion, ConversionRule, DamageKinds, DamageTypeTable, Penetration, merge_types,
};
use crate::foundation::tags::Tags;
use crate::foundation::trace::{TraceKind, trace};
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
/// Resolving one, now or queued:
///
/// 1. Each part takes its own types, else the request's; a part of 0 drops.
///    No part left, or a target that is gone, being despawned by the kit,
///    without [`Health`], dead or, unless the request is periodic, inside
///    its invulnerability window, is [`Hit::Ignored`], with no side effect.
/// 2. Conversion: the hit's own rules, then its source's [`Conversion`],
///    in one pass, each share a new part of its rule's type alone; parts of
///    equal types then merge in first-appearance order.
/// 3. Penetration: each part's points, the hit's own [`Penetration`]'s and
///    its source's matching entries, summed and capped at 1.
/// 4. The target's [`Resist`], per part: a matching factor of 0 drops the
///    part before any product; a factor in `(0, 1)` gains the part's points,
///    capped at 1; the factors multiply the part in `f64`, clamped to
///    `f32::MAX`, and a result of 0 drops the part.
/// 5. No part left is [`Hit::Ignored`], with no side effect.
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
    crit: bool,
    periodic: bool,
    penetration: Option<Penetration>,
    conversion: Option<Conversion>,
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
            crit: false,
            periodic: false,
            penetration: None,
            conversion: None,
        }
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
        self.conversion = Some(conversion);
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

    /// The request, marked a critical hit when `crit`: the kit only carries
    /// it to [`Damaged`] and to each of its [`DamagedPart`]s.
    #[must_use]
    pub fn with_crit(mut self, crit: bool) -> Self {
        self.crit = crit;
        self
    }

    /// The request, periodic when `periodic`: an effect's payload, which
    /// passes the target's invulnerability window, starts none and starts
    /// no [`HitFlash`](crate::HitFlash); [`Damaged`] and [`DamagedPart`]
    /// carry the flag.
    #[must_use]
    pub fn with_periodic(mut self, periodic: bool) -> Self {
        self.periodic = periodic;
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
        self.conversion.as_ref()
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

    /// Whether it is a critical hit.
    #[must_use]
    pub fn is_crit(&self) -> bool {
        self.crit
    }

    /// Whether it is periodic: an effect's payload.
    #[must_use]
    pub fn is_periodic(&self) -> bool {
        self.periodic
    }

    /// Resolves the request now, so the caller knows the outcome in the
    /// same step; see the type for the steps.
    pub fn apply(self, world: &mut World) -> Hit {
        merge_types(world);
        resolve(world, &self, &mut Vec::new())
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

/// [`DAMAGE_RESOLVE`]'s state between steps: the drained lists and a
/// resolution's landed parts, kept so a step allocates nothing once warm.
#[derive(Default)]
pub(crate) struct Resolver {
    damage: Vec<DamageRequest>,
    heals: Vec<HealRequest>,
    landed: Vec<Landed>,
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
        self.damage
            .sort_by_key(|request| (request.target.id(), request.source.map(Entity::id)));
        for request in self.damage.drain(..) {
            resolve(world, &request, &mut self.landed);
        }
        self.heals
            .sort_by_key(|request| (request.target.id(), request.source.map(Entity::id)));
        for request in self.heals.drain(..) {
            resolve_heal(world, &request);
        }
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
}

/// A part between the conversion and the resistances: its types and its
/// amount in `f64`.
type Working<'a> = (&'a Tags, f64);

/// What the target's defences read of a request: its resistances, the
/// penetration of the hit and of its source, and the type table.
struct Defences<'a> {
    resist: Option<&'a Resist>,
    own: Option<&'a Penetration>,
    lent: Option<&'a Penetration>,
    table: Option<&'a DamageTypeTable>,
}

impl Defences<'_> {
    /// Steps 3 and 4 for one part: its points, then its factors; `None`
    /// when it drops.
    fn land(&self, tags: &Tags, amount: f64) -> Option<Landed> {
        let amount = match self.resist {
            None => clamped(amount),
            Some(resist) => {
                let points = self.own.map_or(0.0, |own| own.sum(tags))
                    + self.lent.map_or(0.0, |lent| lent.sum(tags));
                clamped(amount * resist.product_with(tags, points.min(1.0))?)
            }
        };
        (amount != 0.0).then(|| Landed {
            amount,
            kinds: self
                .table
                .map_or(DamageKinds::NONE, |table| table.mask(tags)),
        })
    }
}

/// Steps 1 to 4 of `request`'s resolution: its parts after the
/// conversion, the penetration and the target's resistances, in part
/// order, into `landed`; empty when every part dropped.
fn land_parts<'a>(world: &'a World, request: &'a DamageRequest, landed: &mut Vec<Landed>) {
    landed.clear();
    let defences = Defences {
        resist: world.get::<Resist>(request.target),
        own: request.penetration.as_ref(),
        lent: request
            .source
            .and_then(|source| world.get::<Penetration>(source)),
        table: world.get_resource::<DamageTypeTable>(),
    };
    let own_rules = request
        .conversion
        .as_ref()
        .map_or(&[][..], Conversion::rules);
    let lent_rules = request
        .source
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
        if part.amount != 0.0
            && let Some(part) = defences.land(typed(part), f64::from(part.amount))
        {
            landed.push(part);
        }
        return;
    }
    let mut working: Vec<Working<'_>> = Vec::with_capacity(parts.len() + 1);
    for part in parts.iter().filter(|part| part.amount != 0.0) {
        convert(
            typed(part),
            f64::from(part.amount),
            own_rules.iter().chain(lent_rules),
            &mut working,
        );
    }
    for &(tags, amount) in &working {
        if let Some(part) = defences.land(tags, amount.min(f64::from(f32::MAX))) {
            landed.push(part);
        }
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

/// [`DamageRequest`]'s resolution, its landed parts gathered in `landed`.
fn resolve(world: &mut World, request: &DamageRequest, landed: &mut Vec<Landed>) -> Hit {
    let target = request.target;
    if request.parts().iter().all(|part| part.amount == 0.0)
        || !takes_hits(world, target, request.periodic)
    {
        return Hit::Ignored;
    }
    land_parts(world, request, landed);
    if landed.is_empty() {
        return Hit::Ignored;
    }
    let amount = clamped(landed.iter().map(|part| f64::from(part.amount)).sum());
    let kinds = landed
        .iter()
        .fold(DamageKinds::NONE, |kinds, part| kinds.union(part.kinds));
    let rest = match world.get_mut::<Shield>(target) {
        Some(shield) => amount - shield.take(amount),
        None => amount,
    };
    let killed = world
        .get_mut::<Health>(target)
        .unwrap()
        .hurt(rest, request.periodic);
    if !request.periodic {
        start_flash(world, target);
    }
    if world.has_event::<DamagedPart>() {
        let queue = world.resource_mut::<EventQueue<DamagedPart>>();
        for part in landed.iter() {
            let mut event = DamagedPart::new(target, part.amount)
                .with_kinds(part.kinds)
                .with_periodic(request.periodic)
                .with_crit(request.crit);
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
            .with_crit(request.crit)
            .with_periodic(request.periodic)
            .with_kinds(kinds);
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
