//! Combat: 01's damage model on the kit's damage types, `damage` and under
//! it `damage.physical` (with `damage.physical.bleed`), `damage.fire`,
//! `damage.cold` and `damage.shock`, each with its colour and its ailment
//! ladder (`data/damage_types/game.json`). A hit starts from its attack's
//! base amounts, doubles on a crit, then takes the kit's pipeline: the
//! target's resistances, with shocked's half again on every type, then its
//! shield and its health. The ladders put burning, chilled, frozen,
//! shocked, stunned and bleeding on whatever has health; 01 reads their
//! `status.*` tags for its looks and its physics.
//!
//! Combat also floats the damage numbers ([`numbers`]) and toasts each
//! reaction's hint the first time a save sees it: a freeze, a shatter, a
//! steam burst and a shock, which the element rules and the reactions send
//! as [`Reaction`] events.
//!
//! The frame rule: every request is built and queued in the intent, move or
//! overlap phase, so the step's resolve takes it. The kit's queues are not
//! saved, so a request queued after the resolve would not survive a save
//! between two frames.

pub mod components;
pub mod draw;
pub mod numbers;
pub mod systems;

use glam::Vec2;
use tungsten::core::{Entity, EventCursor, EventQueue, Schedule, Stage, World, system};
use tungsten::kit::{
    DAMAGE_RESOLVE, DamagePart, DamageRequest, KitRng, Penetration, Phase, Tags, ensure_phases,
};
use tungsten::plugins::{PARTICLE_COUNT_REFRESH, PHYSICS_SYNC};
use tungsten::{Extracts, Plugin};

use crate::features::enemies::beetle;
use crate::features::run::Run;
use components::{Hints, Reaction};

/// The share of the player's hits that crit, and a crit's factor on every
/// part (Q8).
pub const CRIT_CHANCE: f32 = 0.05;
pub const CRIT_FACTOR: f32 = 2.0;
/// The share of a blast or a steam burst the player takes (Q12).
pub const PLAYER_BLAST_SHARE: f32 = 0.5;
/// What the sprays deal a second to a target in their cone, in pulses this
/// far apart.
pub const SPRAY_DPS: f32 = 30.0;
pub const SPRAY_PULSE: f32 = 0.1;
/// The fire missile's blast: its fire at its centre, half of it at the
/// heat's reach.
pub const BLAST_FIRE: f32 = 20.0;

/// The status tags 01 reads, which the kit's effects grant.
pub const BURNING: &str = "status.burning";
pub const BLEEDING: &str = "status.bleeding";
pub const CHILLED: &str = "status.chilled";
pub const FROZEN: &str = "status.frozen";
pub const SHOCKED: &str = "status.shocked";
pub const STUNNED: &str = "status.stunned";
pub const DOUSED: &str = "status.doused";

/// One of 01's damage types, by its tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Physical,
    Bleed,
    Fire,
    Cold,
    Shock,
}

impl Kind {
    /// Its tag, a declared damage type.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Kind::Physical => "damage.physical",
            Kind::Bleed => "damage.physical.bleed",
            Kind::Fire => "damage.fire",
            Kind::Cold => "damage.cold",
            Kind::Shock => "damage.shock",
        }
    }
}

/// A hit 01 builds, the hit builder: its target, its parts' base amounts,
/// the entity it comes from, where it lands and its push. [`Hit::request`]
/// turns it into the kit's request: a hit from the player crits on one
/// [`KitRng`] draw under [`CRIT_CHANCE`], doubling every part; a blast's or
/// a steam burst's on the player takes [`PLAYER_BLAST_SHARE`]; one landing
/// on a beetle's rear half, or one made whole, takes physical penetration
/// 1, so its shell's physical resistance takes nothing.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    target: Entity,
    parts: Vec<(Kind, f32)>,
    source: Option<Entity>,
    at: Option<Vec2>,
    push: Option<Vec2>,
    blast: bool,
    whole: bool,
}

impl Hit {
    /// A hit on `target` of `parts`, from nobody.
    #[must_use]
    pub fn new(target: Entity, parts: impl IntoIterator<Item = (Kind, f32)>) -> Self {
        Self {
            target,
            parts: parts.into_iter().collect(),
            source: None,
            at: None,
            push: None,
            blast: false,
            whole: false,
        }
    }

    /// The hit, from `source`, who gets the credit.
    #[must_use]
    pub fn from(mut self, source: Entity) -> Self {
        self.source = Some(source);
        self
    }

    /// The hit, landing at `point`.
    #[must_use]
    pub fn at(mut self, point: Vec2) -> Self {
        self.at = Some(point);
        self
    }

    /// The hit, pushing its target by `velocity`.
    #[must_use]
    pub fn push(mut self, velocity: Vec2) -> Self {
        self.push = Some(velocity);
        self
    }

    /// The hit, a blast's or a steam burst's, which the player takes half
    /// of.
    #[must_use]
    pub fn blast(mut self) -> Self {
        self.blast = true;
        self
    }

    /// The hit, landing whole wherever it lands, as on a beetle's rear: an
    /// iron crush.
    #[must_use]
    pub fn whole(mut self) -> Self {
        self.whole = true;
        self
    }

    /// The kit's request: the base amounts, doubled on a crit (one draw,
    /// for a hit from the run's player), halved on the player for a blast,
    /// with physical penetration 1 when whole or on a beetle's rear half.
    pub fn request(&self, world: &mut World) -> DamageRequest {
        let player = world.get_resource::<Run>().map(|run| run.player);
        let crit = self.source.is_some() && self.source == player && roll_crit(world);
        let mut factor = if crit { CRIT_FACTOR } else { 1.0 };
        if self.blast && Some(self.target) == player {
            factor *= PLAYER_BLAST_SHARE;
        }
        let parts = self.parts.iter().map(|&(kind, amount)| {
            DamagePart::new((amount * factor).max(0.0)).with_tags(Tags::new([kind.tag()]))
        });
        let mut request = DamageRequest::from_parts(self.target, parts).with_crit(crit);
        if let Some(source) = self.source {
            request = request.with_source(source);
        }
        if let Some(at) = self.at {
            request = request.with_position(at);
        }
        if let Some(push) = self.push {
            request = request.with_knockback(push);
        }
        let rear = self
            .at
            .is_some_and(|at| beetle::rear(world, self.target, at));
        if self.whole || rear {
            let pen = Penetration::new().with_points(Kind::Physical.tag(), 1.0);
            request = request.with_penetration(pen);
        }
        request
    }

    /// Builds the request and queues it for the step's resolve.
    pub fn queue(self, world: &mut World) -> bool {
        let request = self.request(world);
        request.queue(world)
    }
}

/// One draw of the kit's stream: whether it lands under [`CRIT_CHANCE`].
fn roll_crit(world: &mut World) -> bool {
    world
        .get_resource_mut::<KitRng>()
        .is_some_and(|rng| rng.unit() < CRIT_CHANCE)
}

/// Sends `reaction`, whose hint the run toasts the first time.
pub fn react(world: &mut World, reaction: Reaction) {
    if let Some(queue) = world.get_resource_mut::<EventQueue<Reaction>>() {
        queue.send(reaction);
    }
}

/// `combat_hints`, in the react phase after the reactions and the dummies'
/// ends: each reaction's first in the save toasts its hint, once.
pub fn hints(world: &mut World, cursor: &mut EventCursor<Reaction>) {
    let fresh: Vec<Reaction> = world
        .get_resource::<EventQueue<Reaction>>()
        .map(|events| cursor.read(events).copied().collect())
        .unwrap_or_default();
    for reaction in fresh {
        let Some(hints) = world.get_resource_mut::<Hints>() else {
            return;
        };
        if hints.seen.contains(&reaction) {
            continue;
        }
        hints.seen.push(reaction);
        crate::shell::toast::toast(world, reaction.hint());
    }
}

/// Whether `entity` carries the status `tag`.
#[must_use]
pub fn has_status(world: &World, entity: Entity, tag: &str) -> bool {
    world.get::<Tags>(entity).is_some_and(|tags| tags.has(tag))
}

/// Combat as a plugin: in the fixed step, the states the resolve begins
/// with, recorded before the kit's damage resolve, and in the react phase
/// the reactions to the step's landed parts, the damage numbers and the
/// hints; in the present phase the statuses' tints, before the frame's
/// particles their emitters, and their overlays drawn after the sprites.
pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        world.resource_mut::<Extracts>().add_sprites(draw::overlays);
        world.register_event::<Reaction>();
        ensure_phases(schedule);
        let states = system("combat_states", systems::states).before(DAMAGE_RESOLVE);
        schedule.add(Stage::FixedUpdate, Phase::Resolve.join(states));
        let (mut parts, mut applied) = (EventCursor::new(), EventCursor::new());
        let reactions = system("combat_reactions", move |world: &mut World| {
            systems::reactions(world, &mut parts, &mut applied);
        });
        schedule.add(Stage::FixedUpdate, Phase::React.join(reactions));
        let mut parts = EventCursor::new();
        let numbers = system("damage_numbers", move |world: &mut World| {
            numbers::numbers(world, &mut parts);
        });
        schedule.add(Stage::FixedUpdate, Phase::React.join(numbers));
        let mut seen = EventCursor::new();
        let hints = system("combat_hints", move |world: &mut World| {
            hints(world, &mut seen);
        })
        .after("combat_reactions")
        .after("dummy_deaths");
        schedule.add(Stage::FixedUpdate, Phase::React.join(hints));
        schedule.add(
            Stage::PostUpdate,
            Phase::Present.join(system("status_tints", systems::tints)),
        );
        let particles = system("status_particles", systems::particles)
            .after(PHYSICS_SYNC)
            .before(PARTICLE_COUNT_REFRESH);
        schedule.add(Stage::PostUpdate, particles);
    }
}
