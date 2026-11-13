//! The element rules of 01's bodies, its balls, iron and scraps, which
//! have no health, on the kit's statuses (7.13a): small balls burn for ten
//! seconds (`ball_burn`, with a lasting `kindled` that marks them `spent`),
//! spreading by contact and nearness, and never light again once spent;
//! cold builds a coat on iron and balls (`chill`, an intensity the kit
//! wears down, through chemistry's `cold` rule) until they freeze, for
//! five seconds (`ice`); the black hole puts out what burns in its reach; fast
//! iron crushes small balls and hurts the player with physical damage; a
//! hard impact bursts a small ball's sparks; moving bodies hurt targets by
//! their speed and mass; cold puts a fire hazard out for a while. What has
//! health takes the kit's damage types, effects and chemistry's builds
//! instead (`combat`).
//!
//! The endings, one rule each for any frozen thing, run through the saved
//! [`cascade`] queue: fire on a frozen thing explodes it (thermal shock), a
//! hard impact or a fall shatters it, and cold on a burning thing bursts
//! into steam, which chains through a burning pile.

pub mod cascade;
pub mod components;
pub mod draw;
pub mod systems;

use glam::Vec2;
use tungsten::core::save::SaveOptions;
use tungsten::core::{Entity, Schedule, Stage, World, system};
use tungsten::kit::{PROJECTILE_TICK, Phase, ensure_phases};
use tungsten::plugins::{PARTICLE_COUNT_REFRESH, PHYSICS_SYNC};
use tungsten::{Extracts, Plugin};

use crate::features::combat::{BURNING, FROZEN, has_status};
use crate::shared::SaveId;

/// A small ball's burn, game seconds.
pub const BURN_SECS: f32 = 10.0;
/// The bodies' statuses by their `effects` IDs: a small ball's burn,
/// granting `status.burning` for [`BURN_SECS`], and its lasting `kindled`,
/// granting [`SPENT`]; a body's coat of frost, an intensity to 1 that wears
/// at [`THAW_RATE`] a second, and its freeze, granting `status.frozen` for
/// [`FROZEN_SECS`]. The burn and the ice last 1 s scaled by their
/// request's scale, their seconds (a line through 0 and 16 on the
/// duration, which the kit's f64 arithmetic keeps exact), so a load gives
/// one back the seconds it saved, to the bit.
pub const BALL_BURN: &str = "ball_burn";
pub const KINDLED: &str = "kindled";
pub const COAT: &str = "chill";
pub const ICE: &str = "ice";
/// The tag a ball that has burned carries for good.
pub const SPENT: &str = "spent";
/// 01's own stimulus a cold spray sends a body it meets, which chemistry's
/// `coat` rule builds the coat by.
pub const COLD: &str = "cold";
/// 01's own stimuli for its bodies: `heat`, which explodes a frozen one
/// (thermal shock) and lights the rest, and `crack`, a hard hit's shards
/// on a frozen thing; chemistry's rules decide each. `crack` sorts before
/// the damage types, so the kit's order (by name among one receiver's
/// entries of an age and a depth) decides a blow's crack before its own
/// hit's parts, as the cascade ended a frozen thing before the hit that
/// shattered it could build on it.
pub const HEAT: &str = "heat";
pub const CRACK: &str = "crack";
/// 01's own stimuli for its props (7.13a): `kindle`, a fire's spread,
/// which lights what is not frozen and leaves a frozen prop be, as `heat`
/// would not; and `jolt`, which sets a keg or a bomb off at once, lit or
/// not, and shatters one frozen meanwhile.
pub const KINDLE: &str = "kindle";
pub const JOLT: &str = "jolt";
/// Seconds of cold a body takes to freeze through, and its chill's thaw a
/// second; a frozen body thaws after [`FROZEN_SECS`] (Q17).
pub const FREEZE_TIME: f32 = 1.5;
pub const THAW_RATE: f32 = 0.15;
pub const FROZEN_SECS: f32 = 5.0;
/// Thermal shock's reach by what explodes: an ice ball, frozen iron, a
/// frozen scrap and a frozen target; its fire and physical parts are an
/// eighth of the reach each, and its push at its centre.
pub const BALL_BLAST: f32 = 48.0;
pub const IRON_BLAST: f32 = 160.0;
pub const SCRAP_BLAST: f32 = 64.0;
pub const TARGET_BLAST: f32 = 128.0;
pub const BLAST_PUSH: f32 = 900.0;
/// A steam burst's reach, a burning small ball's, its fire and physical
/// parts and its push at its centre.
pub const STEAM_REACH: f32 = 128.0;
pub const SMALL_STEAM_REACH: f32 = 96.0;
pub const STEAM_PART: f32 = 5.0;
pub const STEAM_PUSH: f32 = 700.0;
/// The most cascades a run takes, and the most that wait (Q11).
pub const CASCADE_CAP: usize = 48;
pub const CASCADE_BOUND: usize = 16_384;
/// A frozen target landing at [`SHATTER_SPEED`] or more takes this share of
/// the speed past it. A moving body's hit on a target is its prefab's
/// `impact_damage`: from 400 px/s, 0.002 × its mass × the speed past it, at
/// most 120 a hit and nothing under 2, a body lighter than [`LIGHT_MASS`]
/// at half its mass, so a small ball hurts from 2,400 px/s, a goblin's bomb
/// from 1,067, a bowling ball from 525 and a crate from 483.
pub const LANDING_SHARE: f32 = 0.1;
/// The lightest body a keg's blow counts whole ([`blows_keg`]), under which
/// an impact counts half its mass.
pub const LIGHT_MASS: f32 = 4.0;
/// The least mass that shatters a frozen body it meets hard: a bowling
/// ball's, so small balls flying in a pile break no ice.
pub const SHATTER_MASS: f32 = 8.0;
/// The closing speed that sets a keg off ([`blows_keg`]): a keg dropped
/// about 150 px onto the ground.
pub const KEG_SPEED: f32 = 1050.0;
/// A crate's break (V33, e01sfx): how near a body ends the step to touch
/// it; the least speed toward it, and the least closing speed, a hit needs;
/// and the hitter's mass times its speed that breaks it: a bowling ball at
/// its 700 px/s toss, a crate at 467, a keg at 350, scrap and iron at the
/// floor. A ball tossed at exactly 700 px/s stands a hair under it in
/// floats, so the break takes a hundredth of slack.
pub const TOUCH_MARGIN: f32 = 2.0;
pub const BREAK_FLOOR: f32 = 300.0;
pub const CRATE_BREAK: f32 = 5600.0;
pub const BREAK_SLACK: f32 = 0.01;
/// Seconds cold puts a fire hazard out for.
pub const DOUSE_SECS: f32 = 4.0;
/// A fire hazard's douse (7.13a): 1 s scaled by its request's seconds, so a
/// load gives one back the seconds it saved, granting `status.doused`; its
/// end runs `relight`.
pub const HAZARD_DOUSED: &str = "hazard_doused";
/// Most emitters over a burning pile's surface: 64 pairs.
pub const FIRE_EMITTER_CAP: usize = 128;
/// Seconds a pair stays on one ball before it hops.
pub const EMITTER_DWELL: f32 = 0.5;
/// A flame tongue's drawn size: its art is drawn in 2 × 2 pixel blocks.
pub const FLAME_SIZE: f32 = 32.0;
/// A small ball's closing speed into a contact, px/s, past which it bursts,
/// the seconds before it may again, and at most so many bursts a step.
pub const IMPACT_SPEED: f32 = 420.0;
pub const IMPACT_COOLDOWN: f32 = 0.12;
pub const IMPACT_BURSTS_PER_STEP: usize = 4;
/// The black hole's steam puffs a step, spread over what it put out.
pub const DOUSE_BURSTS_PER_STEP: usize = 4;
/// Iron's speed into a body, px/s, from which it hurts the player and
/// crushes small balls, and the closing speed that shatters a frozen body.
pub const CRUSH_SPEED: f32 = 360.0;
pub const SHATTER_SPEED: f32 = 600.0;
/// Each further step of this much speed costs the player another hit's
/// health, and crushes another ball; a hit is a hazard's.
pub const SPEED_PER_HIT: f32 = 600.0;
pub const CRUSH_HIT: f32 = 34.0;
pub const SPEED_PER_BALL: f32 = 60.0;
/// The balls a crush takes at its threshold and at most, and the share of
/// its speed iron keeps through a layer it smashed.
pub const BASE_SMASH: usize = 4;
pub const SMASH_CAP: usize = 64;
pub const SMASH_KEEP: f32 = 0.9;
/// A fire hazard's touch box, about its centre from its sprite's top-left.
pub const FIRE_HALF: Vec2 = Vec2::new(18.0, 24.0);
pub const FIRE_CENTRE: Vec2 = Vec2::new(32.0, 32.0);

/// The element rules as a plugin. In the fixed step, after the physics
/// step (the kit times the burns, the freezes and the douses): the fires
/// against the balls, the impact bursts, the crushes, the bodies' impacts,
/// the spread and the burning surface and the black hole's dousing, then
/// the cascades' run, last of 01's contact systems and before the kit's
/// resolve. Before the frame's particles, the burning surface's emitters
/// and the impact sparks' colours; in the present phase the cold pulses;
/// and the flames, pulses and ice shells drawn after the sprites.
pub struct ElementsPlugin;

/// The overlap systems the cascades' run follows.
const CONTACTS: [&str; 7] = [
    "fire_contacts",
    "ball_impacts",
    "iron_crush",
    "body_impacts",
    "fire_spread",
    "fire_exposure",
    "hole_douse",
];

impl Plugin for ElementsPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        world.resource_mut::<Extracts>().add_sprites(draw::effects);
        world.register_saved_resource_with(
            "cascades",
            SaveOptions::<cascade::Cascades>::form::<cascade::CascadesForm>()
                .refs(cascade::cascades_refs())
                .capture_as(cascade::cascades_saved),
        );
        ensure_phases(schedule);
        let fixed = [
            Phase::Overlap.join(system(CONTACTS[0], systems::fire_contacts)),
            Phase::Overlap.join(system(CONTACTS[1], systems::ball_impacts)),
            Phase::Overlap.join(system(CONTACTS[2], systems::crush)),
            Phase::Overlap.join(system(CONTACTS[3], systems::body_impacts)),
            Phase::Overlap.join(system(CONTACTS[4], systems::spread_fire)),
            Phase::Overlap.join(system(CONTACTS[5], systems::flame_exposure)),
            Phase::Overlap.join(system(CONTACTS[6], systems::douse)),
        ];
        for desc in fixed {
            schedule.add(Stage::FixedUpdate, desc);
        }
        let mut run = system("cascade_run", cascade::run)
            .after(PROJECTILE_TICK)
            .after("missile_ends");
        for name in CONTACTS {
            run = run.after(name);
        }
        schedule.add(Stage::FixedUpdate, Phase::Overlap.join(run));
        schedule.add(
            Stage::PostUpdate,
            Phase::Present.join(system("ice_pulses", systems::age_pulses)),
        );
        // Before the frame's emission, which they place and colour.
        for desc in [
            system("fire_particles", systems::fire_particles),
            system("impact_sparks", systems::impact_sparks),
        ] {
            let desc = desc.after(PHYSICS_SYNC).before(PARTICLE_COUNT_REFRESH);
            schedule.add(Stage::PostUpdate, desc);
        }
    }
}

/// Lights `ball` when it is a small ball that never burned.
pub fn ignite(world: &mut World, ball: Entity) {
    systems::ignite(world, ball);
}

/// Chills a body, iron or a ball, by `amount` of its coat this step, at
/// `contact` where the cold met it: chemistry's `cold`, frozen at a full
/// coat, with its cue.
pub fn chill(world: &mut World, body: Entity, amount: f32, contact: Vec2) {
    systems::chill(world, body, amount, contact);
}

/// Freezes a body, iron or a ball, through at once, for [`FROZEN_SECS`].
pub fn freeze_iron(world: &mut World, body: Entity) {
    systems::freeze_iron(world, body);
}

/// Whether `body` is frozen: it carries `status.frozen`.
#[must_use]
pub fn is_frozen(world: &World, body: Entity) -> bool {
    has_status(world, body, FROZEN)
}

/// Whether `ball` burns: it carries `status.burning`.
#[must_use]
pub fn is_burning(world: &World, ball: Entity) -> bool {
    has_status(world, ball, BURNING)
}

/// The game seconds `ball`'s burn has left: its `ball_burn`'s, 0 once it
/// is spent, `None` for one never lit.
#[must_use]
pub fn burn_left(world: &World, ball: Entity) -> Option<f32> {
    systems::burn_left(world, ball)
}

/// Whether a keg meeting something of `mass` (infinite for a tile or a
/// body that does not move) at `closing` px/s goes off: at [`KEG_SPEED`]
/// against anything of [`SHATTER_MASS`] or more, a bowling ball, iron or
/// the ground; faster in proportion against a lighter body; never against
/// one under [`LIGHT_MASS`], a small ball or a goblin's bomb (Q46).
#[must_use]
pub fn blows_keg(mass: f32, closing: f32) -> bool {
    mass >= LIGHT_MASS && closing * (mass / SHATTER_MASS).min(1.0) >= KEG_SPEED
}

/// How frozen a body is, 0 to 1: 1 frozen, else its coat.
#[must_use]
pub fn freeze_amount(world: &World, body: Entity) -> f32 {
    if is_frozen(world, body) {
        1.0
    } else {
        systems::coat_of(world, body)
    }
}

/// The game seconds `body`'s freeze has left: its `ice`'s; `None` unfrozen.
#[must_use]
pub fn ice_left(world: &World, body: Entity) -> Option<f32> {
    systems::status_left(world, body, ICE)
}

/// A blast's heat at `at`: each frozen body whose surface lies within
/// `radius` of it explodes in its turn (thermal shock, [`cascade`]).
pub fn thermal_shatter(world: &mut World, at: Vec2, radius: f32) {
    systems::thermal_shatter(world, at, radius);
}

/// Puts the fire hazard `hazard` out for [`DOUSE_SECS`], with steam.
pub fn douse_hazard(world: &mut World, hazard: Entity) {
    systems::douse_hazard(world, hazard);
}

/// Whether the fire hazard `hazard` is out.
#[must_use]
pub fn is_doused(world: &World, hazard: Entity) -> bool {
    systems::is_doused(world, hazard)
}

/// A cold pulse at `at`, `size` across: a freeze's or a shatter's cue.
pub fn ice_pulse(world: &mut World, at: Vec2, size: f32, shatter: bool) {
    systems::ice_pulse(world, at, size, shatter);
}

/// A burning or spent ball's tint at level time `time`, by its save ID,
/// its burn's seconds `left`.
#[must_use]
pub fn ember_tint(id: SaveId, left: f32, time: f32) -> [u8; 4] {
    systems::ember_tint(id, left, time)
}
