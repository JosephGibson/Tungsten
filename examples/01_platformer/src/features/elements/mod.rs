//! The element rules of 01's bodies, its balls, iron and scraps, which
//! have no health and keep compact components of their own (Q1): small
//! balls burn for ten seconds, spreading by contact and nearness, and never
//! light again once spent; cold chills iron and balls until they freeze,
//! for five seconds; the black hole puts out what burns in its reach; fast
//! iron crushes small balls and hurts the player with physical damage; a
//! hard impact bursts a small ball's sparks; moving bodies hurt targets by
//! their speed and mass; cold puts a fire hazard out for a while. What has
//! health takes the kit's damage types, effects and ailment ladders
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
use tungsten::core::{Entity, Schedule, Stage, World, system};
use tungsten::kit::{PROJECTILE_TICK, Phase, ensure_phases};
use tungsten::plugins::{PARTICLE_COUNT_REFRESH, PHYSICS_SYNC};
use tungsten::{Extracts, Plugin};

use crate::shared::SaveId;
use components::{BallBurn, Chill, Frozen};

/// A small ball's burn, game seconds.
pub const BURN_SECS: f32 = 10.0;
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
/// A moving body's hit on a target: from this closing speed, `k` × mass ×
/// the speed past it, at most so much a hit; a frozen target landing at
/// [`SHATTER_SPEED`] or more takes this share of the speed past it.
pub const BODY_HIT_SPEED: f32 = 400.0;
pub const BODY_HIT_K: f32 = 0.002;
pub const BODY_HIT_MOST: f32 = 120.0;
pub const LANDING_SHARE: f32 = 0.1;
/// The least mass that shatters a frozen body it meets hard: a bowling
/// ball's, so small balls flying in a pile break no ice.
pub const SHATTER_MASS: f32 = 8.0;
/// Seconds cold puts a fire hazard out for.
pub const DOUSE_SECS: f32 = 4.0;
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

/// The element rules as a plugin. In the fixed step: the burns', the
/// freezes' and the doused hazards' clocks first; after the physics step,
/// the fires against the balls, the impact bursts, the crushes, the bodies'
/// impacts, the spread and the burning surface and the black hole's
/// dousing, then the cascades' run, last of 01's contact systems and
/// before the kit's resolve; then the chills' thaw. Before the frame's
/// particles, the burning surface's emitters, the impact sparks' colours
/// and the doused hazards' looks; in the present phase the cold pulses;
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
        ensure_phases(schedule);
        let fixed = [
            Phase::Tick.join(system("fire_tick", systems::tick_fire)),
            Phase::Tick.join(system("frozen_tick", systems::tick_frozen)),
            Phase::Tick.join(system("douse_tick", systems::tick_doused)),
            Phase::Overlap.join(system(CONTACTS[0], systems::fire_contacts)),
            Phase::Overlap.join(system(CONTACTS[1], systems::ball_impacts)),
            Phase::Overlap.join(system(CONTACTS[2], systems::crush)),
            Phase::Overlap.join(system(CONTACTS[3], systems::body_impacts)),
            Phase::Overlap.join(system(CONTACTS[4], systems::spread_fire)),
            Phase::Overlap.join(system(CONTACTS[5], systems::flame_exposure)),
            Phase::Overlap.join(system(CONTACTS[6], systems::douse)),
            Phase::React.join(system("iron_thaw", systems::thaw)),
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
            system("doused_look", systems::doused_look),
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

/// Chills a body, iron or a ball, by `amount`, at `contact` where the cold
/// met it: frozen at a full coating, with its cue.
pub fn chill(world: &mut World, body: Entity, amount: f32, contact: Vec2) {
    systems::chill(world, body, amount, contact);
}

/// Freezes a body, iron or a ball, through at once, for [`FROZEN_SECS`].
pub fn freeze_iron(world: &mut World, body: Entity) {
    systems::freeze_iron(world, body);
}

/// How frozen a body is, 0 to 1.
#[must_use]
pub fn freeze_amount(world: &World, body: Entity) -> f32 {
    if world.has::<Frozen>(body) {
        1.0
    } else {
        world.get::<Chill>(body).map_or(0.0, |chill| chill.amount)
    }
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

/// A cold pulse at `at`, `size` across: a freeze's or a shatter's cue.
pub fn ice_pulse(world: &mut World, at: Vec2, size: f32, shatter: bool) {
    systems::ice_pulse(world, at, size, shatter);
}

/// A burning or spent ball's tint at level time `time`, by its save ID.
#[must_use]
pub fn ember_tint(id: SaveId, burn: BallBurn, time: f32) -> [u8; 4] {
    systems::ember_tint(id, burn, time)
}
