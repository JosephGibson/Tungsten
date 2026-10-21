//! The element rules of 01's physics toys, ported from today's game (Q3):
//! small balls burn for ten seconds, spreading by contact and nearness, and
//! never light again once spent; cold chills iron until it freezes; frozen
//! iron shatters into sixteen scraps under a blast's heat or a hard iron
//! impact; the black hole puts out what burns in its reach; fast iron
//! crushes small balls and hurts the player; a hard impact bursts a small
//! ball's sparks. (b) moves burning and freezing onto the kit's damage
//! types, effects and ailment ladder.

pub mod components;
pub mod draw;
pub mod systems;

use glam::Vec2;
use tungsten::core::{Entity, Schedule, Stage, World, system};
use tungsten::kit::{Phase, ensure_phases};
use tungsten::plugins::{PARTICLE_COUNT_REFRESH, PHYSICS_SYNC};
use tungsten::{Extracts, Plugin};

use crate::shared::SaveId;
use components::{BallBurn, Chill, Frozen};

/// A small ball's burn, game seconds.
pub const BURN_SECS: f32 = 10.0;
/// Seconds of cold iron takes to freeze through, and its thaw a second.
pub const FREEZE_TIME: f32 = 1.5;
pub const THAW_RATE: f32 = 0.15;
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
/// crushes small balls, and its head-on speed that shatters frozen iron.
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

/// The element rules as a plugin. In the fixed step: the burns' and the
/// chills' clocks first; after the physics step, the fires against the
/// balls, the impact bursts, the crushes, the spread and the burning
/// surface and the black hole's dousing; then the thaw. Before the frame's
/// particles, the burning surface's emitters and the impact sparks'
/// colours; in the present phase the cold pulses; and the flames and
/// pulses drawn after the sprites.
pub struct ElementsPlugin;

impl Plugin for ElementsPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        world.resource_mut::<Extracts>().add_sprites(draw::effects);
        ensure_phases(schedule);
        let fixed = [
            Phase::Tick.join(system("fire_tick", systems::tick_fire)),
            Phase::Overlap.join(system("fire_contacts", systems::fire_contacts)),
            Phase::Overlap.join(system("ball_impacts", systems::ball_impacts)),
            Phase::Overlap.join(system("iron_crush", systems::crush)),
            Phase::Overlap.join(system("fire_spread", systems::spread_fire)),
            Phase::Overlap.join(system("fire_exposure", systems::flame_exposure)),
            Phase::Overlap.join(system("hole_douse", systems::douse)),
            Phase::React.join(system("iron_thaw", systems::thaw)),
        ];
        for desc in fixed {
            schedule.add(Stage::FixedUpdate, desc);
        }
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

/// Chills iron by `amount`, at `contact` where the cold met it: frozen at
/// a full coating, with its cue.
pub fn chill(world: &mut World, iron: Entity, amount: f32, contact: Vec2) {
    systems::chill(world, iron, amount, contact);
}

/// Freezes iron through at once.
pub fn freeze_iron(world: &mut World, iron: Entity) {
    systems::freeze_iron(world, iron);
}

/// How frozen iron is, 0 to 1.
#[must_use]
pub fn freeze_amount(world: &World, iron: Entity) -> f32 {
    if world.has::<Frozen>(iron) {
        1.0
    } else {
        world.get::<Chill>(iron).map_or(0.0, |chill| chill.amount)
    }
}

/// Shatters the frozen iron whose surface lies within `radius` of `at`, a
/// blast's heat: a block into sixteen scraps, a scrap into chips.
pub fn thermal_shatter(world: &mut World, at: Vec2, radius: f32) {
    systems::thermal_shatter(world, at, radius);
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
