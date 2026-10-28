//! Combat: 01's damage model on the kit's damage types, `damage` and under
//! it `damage.physical` (with `damage.physical.bleed`), `damage.fire`,
//! `damage.cold`, `damage.shock` and `damage.poison`, each with its colour
//! and its ailment ladder (`data/damage_types/game.json`). A hit starts
//! from its attack's base amounts, doubles on a crit, then takes the kit's
//! pipeline: the target's resistances, with shocked's half again on every
//! type, then its shield and its health. The ladders put burning, chilled,
//! frozen, shocked, stunned and bleeding on whatever has health, and
//! poisoned on what is `living`; 01 reads their `status.*` tags for its
//! looks and its physics.
//!
//! Combat also floats the damage numbers ([`numbers`]).
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
use tungsten::core::{Entity, EventCursor, Schedule, Stage, World, system};
use tungsten::kit::{
    DAMAGE_RESOLVE, DamagePart, DamageRequest, KitRng, Penetration, Phase, Stats, Tags,
    ensure_phases,
};
use tungsten::plugins::{PARTICLE_COUNT_REFRESH, PHYSICS_SYNC};
use tungsten::{Extracts, Plugin};

use crate::features::enemies::{self, armour};
use crate::features::run::Run;

/// The share of the player's hits that crit, and a crit's factor on every
/// part (Q8).
pub const CRIT_CHANCE: f32 = 0.05;
pub const CRIT_FACTOR: f32 = 2.0;
/// The share of a blast or a steam burst the player takes (Q12).
pub const PLAYER_BLAST_SHARE: f32 = 0.5;
/// What the sprays deal a second to a target in their cone, in pulses this
/// far apart: the frost half the flame's, its ladder's gates half the
/// others' (`damage.cold` in `data/damage_types/game.json`), so it chills
/// and freezes as fast.
pub const COLD_SPRAY_DPS: f32 = 15.0;
pub const FIRE_SPRAY_DPS: f32 = 30.0;
pub const SPRAY_PULSE: f32 = 0.1;
/// The cold ladder's chill rung and freeze rung, their shares of a target's
/// threshold: the frost climbs to the chill's before it, and the chill's
/// grade climbs to the freeze's after it ([`systems::frost_level`],
/// [`systems::chill_grade`]).
pub const CHILL_NEEDS: f32 = 0.15;
pub const FREEZE_NEEDS: f32 = 0.5;
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
pub const POISONED: &str = "status.poisoned";
pub const CORRODED: &str = "status.corroded";
/// The kinds' tags 01 reads: what bleeds, what freezes, what poison takes
/// (every enemy but the wisp, and the dummy), and what wears armour (the
/// beetle and the armoured troll).
pub const FLESH: &str = "flesh";
pub const FREEZABLE: &str = "freezable";
pub const LIVING: &str = "living";
pub const ARMOURED: &str = "armoured";

/// One of 01's damage types, by its tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Physical,
    Bleed,
    Fire,
    Cold,
    Shock,
    Poison,
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
            Kind::Poison => "damage.poison",
        }
    }
}

/// A hit 01 builds, the hit builder: its target, its parts' base amounts,
/// the entity it comes from, where it lands and its push. [`Hit::request`]
/// turns it into the kit's request: a hit from the player crits on one
/// [`KitRng`] draw under [`CRIT_CHANCE`], doubling every part; a blast's or
/// a steam burst's on the player takes [`PLAYER_BLAST_SHARE`]; one landing
/// on an armoured creature's rear half, or one made whole, takes physical
/// penetration 1, so its armour's physical resistance takes nothing; and one on a kind
/// that resists knockback whole carries no push (`enemies::stands_firm`).
/// A spell's own hit ([`Hit::spell`]) takes its source's [`SPELL_POWER`]
/// too, which arcane power doubles; the build-up follows the damage.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    target: Entity,
    parts: Vec<(Kind, f32)>,
    source: Option<Entity>,
    at: Option<Vec2>,
    push: Option<Vec2>,
    blast: bool,
    whole: bool,
    spell: bool,
}

/// The stat a spell's own hit is multiplied by, its source's: 1 without
/// it, 2 under the tome of arcane power.
pub const SPELL_POWER: &str = "spell_power";

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
            spell: false,
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

    /// The hit, landing whole wherever it lands, as on armour's rear: an
    /// iron crush.
    #[must_use]
    pub fn whole(mut self) -> Self {
        self.whole = true;
        self
    }

    /// The hit, a spell's own: the missile's blast, a spray's pulse, the
    /// strike or a jump of its chain, which its source's [`SPELL_POWER`]
    /// multiplies.
    #[must_use]
    pub fn spell(mut self) -> Self {
        self.spell = true;
        self
    }

    /// The kit's request: the base amounts, doubled on a crit (one draw,
    /// for a hit from the run's player), a spell's times its source's
    /// [`SPELL_POWER`], halved on the player for a blast, with physical
    /// penetration 1 when whole or on armour's rear half, and its push
    /// unless the target stands firm.
    pub fn request(&self, world: &mut World) -> DamageRequest {
        let player = world.get_resource::<Run>().map(|run| run.player);
        let crit = self.source.is_some() && self.source == player && roll_crit(world);
        let mut factor = if crit { CRIT_FACTOR } else { 1.0 };
        if self.spell
            && let Some(power) = self
                .source
                .and_then(|source| world.get::<Stats>(source))
                .and_then(|stats| stats.get(SPELL_POWER))
        {
            factor *= power.max(0.0);
        }
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
        if let Some(push) = self.push
            && !enemies::stands_firm(world, self.target)
        {
            request = request.with_knockback(push);
        }
        let rear = self
            .at
            .is_some_and(|at| armour::rear(world, self.target, at));
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

/// Whether `entity` carries the status `tag`.
#[must_use]
pub fn has_status(world: &World, entity: Entity, tag: &str) -> bool {
    world.get::<Tags>(entity).is_some_and(|tags| tags.has(tag))
}

/// Combat as a plugin: in the fixed step, the states the resolve begins
/// with, recorded before the kit's damage resolve, and in the react phase
/// the reactions to the step's landed parts, the thaws and the damage
/// numbers; in the present phase the statuses' tints, before the
/// frame's particles their emitters, and their overlays drawn after the
/// sprites. The chill's grades run in the enemies' intent phase.
pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        world.resource_mut::<Extracts>().add_sprites(draw::overlays);
        world.resource_mut::<Extracts>().add_sprites(draw::edge);
        ensure_phases(schedule);
        let states = system("combat_states", systems::states).before(DAMAGE_RESOLVE);
        schedule.add(Stage::FixedUpdate, Phase::Resolve.join(states));
        let (mut parts, mut immune, mut applied) =
            (EventCursor::new(), EventCursor::new(), EventCursor::new());
        let reactions = system("combat_reactions", move |world: &mut World| {
            systems::reactions(world, &mut parts, &mut immune, &mut applied);
        });
        schedule.add(Stage::FixedUpdate, Phase::React.join(reactions));
        let mut expired = EventCursor::new();
        let thaws = system("combat_thaws", move |world: &mut World| {
            systems::thaws(world, &mut expired);
        });
        schedule.add(Stage::FixedUpdate, Phase::React.join(thaws));
        let mut parts = EventCursor::new();
        let numbers = system("damage_numbers", move |world: &mut World| {
            numbers::numbers(world, &mut parts);
        });
        schedule.add(Stage::FixedUpdate, Phase::React.join(numbers));
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
