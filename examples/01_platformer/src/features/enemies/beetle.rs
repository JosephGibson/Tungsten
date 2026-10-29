//! The rust beetle: a horned, shelled walker on the kit's platformer
//! controller. It walks its ground, turning at ledges and walls; once it
//! sees the player within [`CHARGE_RANGE`] across it paws the ground for
//! its tell, then charges along its heading at [`CHARGE_SPEED`], its
//! `beetle_charge` ability arming its hitbox, which hurts whatever it
//! meets, enemies too, and throws the player. Its charge ploughs the
//! bodies lighter than it out of its way ([`PLOUGH_KICK`]) and presses on
//! against what it can't. Its shell is armour (`armour`, e01flask): its
//! front, the half its heading faces, turns bullets as iron does, while
//! its rear half takes every hit 01 builds whole (`combat::Hit`). Iron that
//! drives into it at the crush speed kills it.

use std::collections::HashMap;

use glam::Vec2;
use tungsten::core::{
    BodyKind, Collider, Entity, Position, RigidBody, Shape, Time, Velocity, World,
};
use tungsten::kit::{PlatformerController, StateMachine, use_ability};
use tungsten::physics::{SpatialFilter, overlap, wake};

use super::components::{Enemy, EnemyKind, EnemyState, Held};
use super::systems::{elapsed, face, go, pace, player, rest, sees, stride};
use super::troll::meets;
use super::{ARRIVED, centre};
use crate::features::audio::{self, components::Sound};
use crate::shared::{SaveId, body_centre, spawn_burst};

/// Its patrol's and its search's share of its run speed.
pub const PATROL_PACE: f32 = 0.6;
pub const SEARCH_PACE: f32 = 0.8;
/// How near the player, across, its chase winds up a charge, and how far
/// above or below it the player may stand.
pub const CHARGE_RANGE: f32 = 360.0;
pub const CHARGE_RISE: f32 = 80.0;
/// The paw before the charge, the charge's speed and its seconds, which
/// its ability's hitbox window matches.
pub const PAW_SECS: f32 = 0.8;
pub const CHARGE_SPEED: f32 = 600.0;
pub const CHARGE_SECS: f32 = 0.8;
/// Its charge's ability, which arms its hitbox.
pub const CHARGE: &str = "beetle_charge";
/// The dust: the paw's particle config at its front foot and the charge's
/// in its trail, and the seconds between two puffs of each.
pub const PAW_DUST: &str = "ex10_paw_dust";
pub const DUST: &str = "ex10_charge_dust";
pub const PAW_DUST_SECS: f32 = 0.15;
pub const TRAIL_DUST_SECS: f32 = 0.06;
/// The plough (K30): the box before its face, [`PLOUGH_HALF`] about its
/// body's `plough` point ([`plough_box`]), and the kick a body
/// there takes along its heading, px/s: [`PLOUGH_KICK`] over its mass, at
/// most [`PLOUGH_MOST`], and [`PLOUGH_RISE`] of it up. A crate's is 1,000,
/// a keg's 750, under the speed that sets a keg off.
pub const PLOUGH_HALF: Vec2 = Vec2::new(12.0, 14.0);
pub const PLOUGH_KICK: f32 = 12_000.0;
pub const PLOUGH_MOST: f32 = 1_000.0;
pub const PLOUGH_RISE: f32 = 0.3;

/// Each beetle's seconds to its next puff of dust while it paws or
/// charges: presentation, not saved, so a load starts each over.
#[derive(Debug, Default)]
pub struct Dust(HashMap<Entity, f32>);

/// Whether `entity` is a beetle.
#[must_use]
pub fn is_beetle(world: &World, entity: Entity) -> bool {
    world
        .get::<Enemy>(entity)
        .is_some_and(|enemy| enemy.kind == EnemyKind::Beetle)
}

/// On patrol it walks its ground, turning at each end and at walls.
pub fn patrol(world: &mut World, beetle: Entity) {
    pace(world, beetle, PATROL_PACE);
}

/// The chase: toward the player while it sees it, pawing for a charge once
/// within [`CHARGE_RANGE`] across and [`CHARGE_RISE`] up or down; it
/// searches once it loses sight.
pub fn chase(world: &mut World, beetle: Entity) {
    if !sees(world, beetle) {
        go(world, beetle, EnemyState::Search);
        return;
    }
    let (Some((_, target)), Some(at)) = (player(world), centre(world, beetle)) else {
        return;
    };
    let grounded = world
        .get::<PlatformerController>(beetle)
        .is_some_and(PlatformerController::grounded);
    let across = target.x - at.x;
    if grounded && across.abs() <= CHARGE_RANGE && (target.y - at.y).abs() <= CHARGE_RISE {
        face(world, beetle, target.x);
        go(world, beetle, EnemyState::WindUp);
        rest(world, beetle);
        return;
    }
    stride(world, beetle, across.signum(), 1.0);
}

/// Its tell: it faces the player and paws the ground.
pub fn enter_windup(world: &mut World, beetle: Entity) {
    if let Some((_, target)) = player(world) {
        face(world, beetle, target.x);
    }
    if let Some(at) = centre(world, beetle) {
        audio::play(world, Sound::BeetlePaw, at);
    }
}

/// The paw lasts [`PAW_SECS`], then the charge.
pub fn windup(world: &mut World, beetle: Entity) {
    rest(world, beetle);
    if elapsed(world, beetle) >= PAW_SECS - 1e-4 {
        go(world, beetle, EnemyState::Attack);
    }
}

/// The charge: along its heading at [`CHARGE_SPEED`], its input locked for
/// [`CHARGE_SECS`] and its hitbox armed for as long by its ability.
pub fn enter_attack(world: &mut World, beetle: Entity) {
    let Some(heading) = world.get::<Enemy>(beetle).map(|mine| mine.heading) else {
        return;
    };
    if let Some(controller) = world.get_mut::<PlatformerController>(beetle) {
        controller.launch(Vec2::new(heading * CHARGE_SPEED, 0.0));
        controller.lock_input(CHARGE_SECS);
    }
    use_ability(world, beetle, CHARGE);
    if let Some(at) = centre(world, beetle) {
        audio::play(world, Sound::BeetleCharge, at);
    }
}

/// The charge, launched again each step at [`CHARGE_SPEED`] along its
/// heading, falling as it falls, under the lock its start set: a wall that
/// stops it stops it a step. Each step it ploughs what lies before its face
/// ([`plough`]); it ends after [`CHARGE_SECS`].
pub fn attack(world: &mut World, beetle: Entity) {
    if elapsed(world, beetle) >= CHARGE_SECS - 1e-4 {
        go(world, beetle, EnemyState::Recover);
        return;
    }
    let Some(heading) = world.get::<Enemy>(beetle).map(|mine| mine.heading) else {
        return;
    };
    let fall = world
        .get::<Velocity>(beetle)
        .map_or(0.0, |velocity| velocity.0.y);
    if let Some(controller) = world.get_mut::<PlatformerController>(beetle) {
        controller.launch(Vec2::new(heading * CHARGE_SPEED, fall));
    }
    plough(world, beetle, heading);
}

/// The plough's box's centre: its body's `plough` point as it faces.
#[must_use]
pub fn plough_box(world: &World, beetle: Entity) -> Option<Vec2> {
    let at = world.get::<Position>(beetle)?.0;
    let heading = world.get::<Enemy>(beetle)?.heading;
    Some(at + super::body(world, EnemyKind::Beetle).point("plough", heading)?)
}

/// Each saved dynamic body, not held, lighter than the beetle, that meets
/// its plough's box, in save-ID order: one slower along `heading` than its
/// kick takes that kick along it and [`PLOUGH_RISE`] of it up, rising no
/// slower, and wakes. Iron, heavier, it presses as a wall, since friction
/// after the step would let each step's kick top it up. The first body a
/// step sets moving, slower than half its kick, sounds.
fn plough(world: &mut World, beetle: Entity, heading: f32) {
    let Some(box_at) = plough_box(world, beetle) else {
        return;
    };
    let shape = Shape::Aabb {
        half_extents: PLOUGH_HALF,
    };
    let filter = SpatialFilter::new().with_tiles(false).excluding(beetle);
    let mut met = Vec::new();
    overlap(world, shape, box_at, &filter, &mut met);
    let heaviest = EnemyKind::Beetle.info().mass;
    let mut bodies: Vec<(u64, Entity, f32)> = met
        .into_iter()
        .flatten()
        .filter_map(|entity| {
            let body = world.get::<RigidBody>(entity)?;
            let collider = world.get::<Collider>(entity)?;
            let at = world.get::<Position>(entity)?.0 + collider.offset;
            let light = body.kind == BodyKind::Dynamic && body.inv_mass * heaviest > 1.0;
            if !light
                || world.has::<Held>(entity)
                || !meets(at, collider.shape, box_at, PLOUGH_HALF)
            {
                return None;
            }
            let kick = (PLOUGH_KICK * body.inv_mass).min(PLOUGH_MOST);
            Some((world.get::<SaveId>(entity)?.0, entity, kick))
        })
        .collect();
    bodies.sort_unstable_by_key(|&(id, ..)| id);
    let mut sounded = false;
    for (_, body, kick) in bodies {
        let Some(velocity) = world.get_mut::<Velocity>(body) else {
            continue;
        };
        let along = velocity.0.x * heading;
        if along >= kick {
            continue;
        }
        velocity.0.x = heading * kick;
        velocity.0.y = velocity.0.y.min(-PLOUGH_RISE * kick);
        wake(world, body);
        if !sounded
            && along < kick / 2.0
            && let Some(at) = body_centre(world, body)
        {
            sounded = true;
            audio::play(world, Sound::BeetlePloughed, at);
        }
    }
}

/// Walks toward `point`'s column: its search's or its home's. Whether it is
/// there, or can go no nearer.
pub fn seek(world: &mut World, beetle: Entity, point: Option<Vec2>) -> bool {
    let (Some(point), Some(at)) = (point, centre(world, beetle)) else {
        rest(world, beetle);
        return true;
    };
    let across = point.x - at.x;
    if across.abs() <= ARRIVED {
        rest(world, beetle);
        return true;
    }
    !stride(world, beetle, across.signum(), SEARCH_PACE)
}

/// Its clip for `state`: the paw through its tell, the charge through its
/// attack, else its walk.
#[must_use]
pub fn clip(state: EnemyState) -> &'static str {
    match state {
        EnemyState::WindUp => "beetle_paw",
        EnemyState::Attack => "beetle_charge",
        _ => "beetle_walk",
    }
}

/// `beetle_dust`, before the frame's particles: a beetle pawing kicks up a
/// puff of [`PAW_DUST`] at its front foot every [`PAW_DUST_SECS`]; one
/// charging leaves a trail of [`DUST`] at its rear feet every
/// [`TRAIL_DUST_SECS`]. Drawn from the state each frame, so a load shows it
/// at once.
pub fn dust(world: &mut World) {
    let dt = world.resource::<Time>().delta();
    if dt <= 0.0 {
        return;
    }
    let beetles: Vec<(Entity, EnemyState, f32)> = world
        .query::<(Entity, &Enemy, &StateMachine<EnemyState>)>()
        .filter(|(_, enemy, _)| enemy.kind == EnemyKind::Beetle)
        .map(|(entity, enemy, machine)| (entity, machine.state(), enemy.heading))
        .collect();
    let mut puffs = Vec::new();
    let mut timers = std::mem::take(&mut world.resource_mut::<Dust>().0);
    timers.retain(|entity, _| {
        beetles.iter().any(|(e, state, _)| {
            e == entity && matches!(state, EnemyState::WindUp | EnemyState::Attack)
        })
    });
    for (beetle, state, heading) in beetles {
        let (every, side, config) = match state {
            EnemyState::WindUp => (PAW_DUST_SECS, 0.6, PAW_DUST),
            EnemyState::Attack => (TRAIL_DUST_SECS, -0.5, DUST),
            _ => continue,
        };
        let next = timers.entry(beetle).or_insert(0.0);
        *next -= dt;
        if *next > 0.0 {
            continue;
        }
        *next += every;
        if let Some(at) = centre(world, beetle) {
            let half = super::body(world, EnemyKind::Beetle).half;
            puffs.push((
                at + Vec2::new(heading * side * half.x, half.y - 4.0),
                config,
            ));
        }
    }
    world.resource_mut::<Dust>().0 = timers;
    for (at, config) in puffs {
        spawn_burst(world, config, at);
    }
}
