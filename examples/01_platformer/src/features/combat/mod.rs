//! Combat: 01's damage model on the kit's damage types, `damage` and under
//! it `damage.physical` (with `damage.physical.bleed`), `damage.fire`,
//! `damage.cold`, `damage.shock` and `damage.poison`, each with its colour
//! (`data/damage_types/game.json`). A hit starts from its attack's base
//! amounts, doubles on a crit, then takes the kit's pipeline: the target's
//! resistances, with shocked's half again on every type, then its shield
//! and its health. Its landed parts are chemistry's stimuli: a type's
//! `interactions` rules build a status a rung (`data/effects/*_build.json`)
//! in shares of the target's [`STATUS_THRESHOLD`], each build's band
//! applying its rung's effect, so burning, chilled, frozen, shocked,
//! stunned and bleeding go on whatever has health, and poisoned on what is
//! `living`; 01 reads their `status.*` tags for its looks and its physics.
//!
//! Its damage numbers are the kit's part labels, the `numbers` cue rule's
//! (`data/cue_rules/numbers.json`), under the game's
//! [`DAMAGE_NUMBERS_FLAG`], which the Damage numbers option sets each step.
//!
//! The frame rule: every request is built and queued in the intent, move or
//! overlap phase, so the step's resolve takes it. The kit saves its queued
//! requests (`damage_requests`) with the World, so one queued after the
//! resolve waits for the next step's, a save between two frames included.

pub mod components;
pub mod draw;
pub mod systems;

use glam::Vec2;
use tungsten::core::{Entity, EventCursor, Schedule, Stage, World, system};
use tungsten::kit::{
    CUES, DAMAGE_RESOLVE, DamagePart, DamageRequest, Flags, Penetration, Phase, Tags, ensure_phases,
};
use tungsten::plugins::{PARTICLE_COUNT_REFRESH, PHYSICS_SYNC};
use tungsten::{Extracts, Plugin};

use crate::features::enemies::armour;
use crate::shell::options::GameOptions;

/// The share of the player's hits that crit, and a crit's factor on every
/// part (Q8): the player's `crit_chance` and `crit_multiplier` in
/// `player.json`, which the kit's rule reads (`DamageRequest::roll_crit`).
pub const CRIT_CHANCE: f32 = 0.05;
pub const CRIT_FACTOR: f32 = 2.0;
/// The share of a blast or a steam burst the player takes (Q12): the
/// player's `damage_taken` line for [`BLAST`] in `player.json`.
pub const PLAYER_BLAST_SHARE: f32 = 0.5;
/// The context a blast's or a steam burst's hit carries, which the player's
/// `damage_taken` halves.
pub const BLAST: &str = "hit.blast";
/// The context a spell's own hit carries, which the caster's
/// `damage_dealt` reads: arcane power doubles it (`tomes`).
pub const SPELL: &str = "hit.spell";
/// What the sprays deal a second to a target in their cone, in pulses this
/// far apart: the frost half the flame's, its builds' decay half the
/// others' (`data/effects/chill_build.json`, `freeze_build.json`), so it
/// chills and freezes as fast.
pub const COLD_SPRAY_DPS: f32 = 15.0;
pub const FIRE_SPRAY_DPS: f32 = 30.0;
pub const SPRAY_PULSE: f32 = 0.1;
/// The cold's chill and freeze, their builds' bands in shares of a target's
/// threshold: the frost climbs to the chill's before it, and the chill's
/// grade climbs to the freeze's after it ([`systems::frost_level`],
/// [`systems::chill_grade`]).
pub const CHILL_NEEDS: f32 = 0.15;
pub const FREEZE_NEEDS: f32 = 0.5;
/// The game stat a target's builds are shares of (`build`'s `per`), its
/// prefab's: what the kit's ladder called its ailment threshold.
pub const STATUS_THRESHOLD: &str = "status_threshold";
/// The cold's builds by their `effects` IDs: the chill's, and the freeze's,
/// the troll's of its own.
pub const CHILL_BUILD: &str = "chill_build";
pub const FREEZE_BUILDS: [&str; 2] = ["freeze_build", "troll_freeze_build"];
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
/// turns it into the kit's request: its crit is the kit's, from its source's
/// stats, so a hit from the player crits on one draw under [`CRIT_CHANCE`],
/// doubling every part ([`CRIT_FACTOR`]); a blast's or a steam burst's
/// carries [`BLAST`] in its context, which the player's `damage_taken`
/// takes [`PLAYER_BLAST_SHARE`] of; a spell's own ([`Hit::spell`]) carries
/// [`SPELL`], which its source's `damage_dealt` reads at the resolve, so
/// arcane power doubles it and the build-up follows the damage; one made
/// whole takes physical penetration 1, and one landing on an armoured
/// creature's rear half takes it from the `armour` hit preparation
/// ([`armour::prepare`]), the rule the kit's blasts and impacts name, so
/// its armour's physical resistance takes nothing; its push goes to the
/// kit's `knock_back`, which a whole resist stands firm against.
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

    /// The hit, a blast's or a steam burst's: its context carries
    /// [`BLAST`], which the player takes half of.
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
    /// strike or a jump of its chain, in the [`SPELL`] context, which its
    /// source's `damage_dealt` reads.
    #[must_use]
    pub fn spell(mut self) -> Self {
        self.spell = true;
        self
    }

    /// The kit's request: the base amounts, its crit rolled by the kit from
    /// its source's stats (one draw, for the player's), a blast's in the
    /// [`BLAST`] context and a spell's in the [`SPELL`] one, its point and
    /// its push, with physical penetration 1 when whole, else the `armour`
    /// preparation's on armour's rear half.
    pub fn request(&self, world: &mut World) -> DamageRequest {
        let parts = self.parts.iter().map(|&(kind, amount)| {
            DamagePart::new(amount.max(0.0)).with_tags(Tags::new([kind.tag()]))
        });
        let mut request = DamageRequest::from_parts(self.target, parts);
        if let Some(source) = self.source {
            request = request.with_source(source);
        }
        request = request.roll_crit(world);
        let context: Vec<&str> = [(self.blast, BLAST), (self.spell, SPELL)]
            .into_iter()
            .filter_map(|(on, tag)| on.then_some(tag))
            .collect();
        if !context.is_empty() {
            request = request.with_context(Tags::new(context));
        }
        if let Some(at) = self.at {
            request = request.with_position(at);
        }
        if let Some(push) = self.push {
            request = request.with_knockback(push);
        }
        if self.whole {
            let pen = Penetration::new().with_points(Kind::Physical.tag(), 1.0);
            request = request.with_penetration(pen);
        } else {
            armour::prepare(world, &mut request);
        }
        request
    }

    /// Builds the request and queues it for the step's resolve.
    pub fn queue(self, world: &mut World) -> bool {
        let request = self.request(world);
        request.queue(world)
    }
}

/// Whether `entity` carries the status `tag`.
#[must_use]
pub fn has_status(world: &World, entity: Entity, tag: &str) -> bool {
    world.get::<Tags>(entity).is_some_and(|tags| tags.has(tag))
}

/// The game's flag the `numbers` cue rule floats a part's damage under (its
/// `flags` filter), set from the Damage numbers option.
pub const DAMAGE_NUMBERS_FLAG: &str = "show.damage_numbers";

/// `damage_numbers_flag`, in the react phase before the kit's cues: the
/// game's [`DAMAGE_NUMBERS_FLAG`] set from the Damage numbers option each
/// step (on without the options), so a change takes the next step's parts.
fn damage_numbers_flag(world: &mut World) {
    let shown = world
        .get_resource::<GameOptions>()
        .is_none_or(|options| options.damage_numbers);
    if !world.has_resource::<Flags>() {
        world.insert_resource(Flags::new());
    }
    let flags = world.resource_mut::<Flags>();
    if flags.has(DAMAGE_NUMBERS_FLAG) != shown {
        flags.set(DAMAGE_NUMBERS_FLAG, shown);
    }
}

/// Combat as a plugin: in the fixed step, the states the resolve begins
/// with, recorded before the kit's damage resolve, and in the react phase
/// the reactions to the step's landed parts, the thaws and the damage
/// numbers' flag; in the present phase the statuses' tints, before the
/// frame's particles their emitters, and their overlays drawn after the
/// sprites. The chill's grades run in the enemies' intent phase.
pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        world.insert_resource(systems::Grades::default());
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
        let flag = system("damage_numbers_flag", damage_numbers_flag).before(CUES);
        schedule.add(Stage::FixedUpdate, Phase::React.join(flag));
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
