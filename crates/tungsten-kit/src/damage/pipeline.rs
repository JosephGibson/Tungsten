//! The damage pipeline: requests that carry a source, damage tags, a
//! position, a knockback and a crit flag, resolved through resistances, a
//! shield and health, at once or once a step in sorted order; heals the
//! same way.

use glam::Vec2;
use tungsten_core::{Entity, EventQueue, World};

use crate::damage::defense::{Resist, Shield};
use crate::damage::health::{Damaged, Health, Hit, start_flash};
use crate::foundation::tags::Tags;
use crate::foundation::trace::{TraceKind, trace};
use crate::lifecycle::ended::Despawning;
use crate::motion::knockback::knock_back;

/// [`HealthPlugin`](crate::HealthPlugin)'s system that resolves the queued
/// requests, in the resolve phase.
pub const DAMAGE_RESOLVE: &str = "damage_resolve";

/// A hit to resolve: its target and amount, with the entity it came from,
/// its damage tags (`damage.fire`), where it landed, the push it gives and
/// whether it was a critical one. The kit's hitboxes, contact damage and
/// projectiles queue one per hit; a game's Rust builds its own and either
/// [`apply`](Self::apply)s it now or [`queue`](Self::queue)s it for the
/// step's [`DAMAGE_RESOLVE`].
///
/// Resolving one, now or queued:
///
/// 1. An amount of 0, or a target that is gone, being despawned by the
///    kit, without [`Health`], dead or inside its invulnerability window,
///    is [`Hit::Ignored`], with no side effect.
/// 2. Its [`Resist`]: a listed tag the request's tags carry with a factor
///    of 0 is immunity, [`Hit::Ignored`]; otherwise the matching factors
///    multiply the amount, which is clamped to `f32::MAX`, and a product of
///    0 is ignored too.
/// 3. Its [`Shield`] takes what it can and waits its delay again.
/// 4. Health falls by the rest; the window and the [`HitFlash`](crate::HitFlash)
///    start; [`Damaged`] goes out with the amount after the resistances, the
///    source, the position and the crit flag; the trace records the hit and
///    any kill.
/// 5. The outcome is [`Hit::Killed`], [`Hit::Hurt`] or [`Hit::Absorbed`]
///    (the shield took it all), and on the last two the knockback goes
///    through [`knock_back`](crate::knock_back).
/// 6. A kill of a health with a death policy starts its delay
///    ([`Health::with_despawn_on_death`]).
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
#[derive(Debug, Clone, PartialEq)]
pub struct DamageRequest {
    target: Entity,
    amount: f32,
    source: Option<Entity>,
    tags: Tags,
    position: Option<Vec2>,
    knockback: Option<Vec2>,
    crit: bool,
}

impl DamageRequest {
    /// A request of `amount` on `target`, with no source, no tags, no
    /// position, no knockback and no crit.
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
        Self {
            target,
            amount,
            source: None,
            tags: Tags::default(),
            position: None,
            knockback: None,
            crit: false,
        }
    }

    /// The request, from `source`: who gets the credit, and the cue
    /// events' other entity.
    #[must_use]
    pub fn with_source(mut self, source: Entity) -> Self {
        self.source = Some(source);
        self
    }

    /// The request, of the damage kinds `tags` (`damage.fire`), which a
    /// [`Resist`] reads.
    #[must_use]
    pub fn with_tags(mut self, tags: Tags) -> Self {
        self.tags = tags;
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
    /// it to [`Damaged`].
    #[must_use]
    pub fn with_crit(mut self, crit: bool) -> Self {
        self.crit = crit;
        self
    }

    /// The entity hit.
    #[must_use]
    pub fn target(&self) -> Entity {
        self.target
    }

    /// The amount before resistances.
    #[must_use]
    pub fn amount(&self) -> f32 {
        self.amount
    }

    /// The entity it came from, when it has one.
    #[must_use]
    pub fn source(&self) -> Option<Entity> {
        self.source
    }

    /// Its damage tags.
    #[must_use]
    pub fn tags(&self) -> &Tags {
        &self.tags
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

    /// Resolves the request now, so the caller knows the outcome in the
    /// same step; see the type for the steps.
    pub fn apply(self, world: &mut World) -> Hit {
        resolve(world, &self)
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

/// [`DAMAGE_RESOLVE`]'s state between steps: the drained lists, kept so a
/// step allocates nothing once warm.
#[derive(Default)]
pub(crate) struct Resolver {
    damage: Vec<DamageRequest>,
    heals: Vec<HealRequest>,
}

impl Resolver {
    /// Resolves the queued damage requests, stable-sorted by target index
    /// and then source index with none first, then the heals the same way.
    pub(crate) fn run(&mut self, world: &mut World) {
        let Some(requests) = world.get_resource_mut::<Requests>() else {
            return;
        };
        if requests.damage.is_empty() && requests.heals.is_empty() {
            return;
        }
        std::mem::swap(&mut self.damage, &mut requests.damage);
        std::mem::swap(&mut self.heals, &mut requests.heals);
        self.damage
            .sort_by_key(|request| (request.target.id(), request.source.map(Entity::id)));
        for request in self.damage.drain(..) {
            resolve(world, &request);
        }
        self.heals
            .sort_by_key(|request| (request.target.id(), request.source.map(Entity::id)));
        for request in self.heals.drain(..) {
            resolve_heal(world, &request);
        }
    }
}

/// Whether `target` takes a request: alive, not being despawned by the kit,
/// with [`Health`] that is neither dead nor inside its window.
fn takes_hits(world: &World, target: Entity) -> bool {
    world.is_alive(target)
        && !world.has::<Despawning>(target)
        && world
            .get::<Health>(target)
            .is_some_and(|health| !health.is_dead() && !health.is_invulnerable())
}

/// The amount `request` deals after `target`'s resistances, in `f64` and
/// clamped to `f32::MAX`; `None` when an immunity ignores it.
fn after_resistances(world: &World, target: Entity, request: &DamageRequest) -> Option<f32> {
    match world.get::<Resist>(target) {
        None => Some(request.amount),
        Some(resist) => {
            let product = resist.product(&request.tags)?;
            Some((f64::from(request.amount) * product).min(f64::from(f32::MAX)) as f32)
        }
    }
}

/// [`DamageRequest`]'s resolution.
fn resolve(world: &mut World, request: &DamageRequest) -> Hit {
    let target = request.target;
    if request.amount == 0.0 || !takes_hits(world, target) {
        return Hit::Ignored;
    }
    let Some(amount) = after_resistances(world, target, request) else {
        return Hit::Ignored;
    };
    if amount == 0.0 {
        return Hit::Ignored;
    }
    let rest = match world.get_mut::<Shield>(target) {
        Some(shield) => amount - shield.take(amount),
        None => amount,
    };
    let killed = world.get_mut::<Health>(target).unwrap().hurt(rest);
    start_flash(world, target);
    if world.has_event::<Damaged>() {
        let mut event = Damaged::new(target, amount, killed).with_crit(request.crit);
        if let Some(source) = request.source {
            event = event.with_source(source);
        }
        if let Some(position) = request.position {
            event = event.with_position(position);
        }
        world.resource_mut::<EventQueue<Damaged>>().send(event);
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
