//! The fire wisp: a flame with a face that flies on the kit's top-down
//! mover within [`ROAM`] of its home, burns what it touches, lights the
//! scene with its glow and hums. Fire is its own: it takes none, so it
//! never burns, and cold freezes it, never steams it. Outside a chase it
//! drifts to the nearest burning ball within [`FIRE_REACH`] and heals by
//! each burning ball within [`HEAL_REACH`].

use glam::Vec2;
use tungsten::SceneEntity;
use tungsten::core::{
    Entity, ParticleConfigRegistry, ParticleEmitter, ParticleEmitterState, Position, World,
};
use tungsten::kit::{HealRequest, Health, StateMachine, TopDownMover, spawn_child};

use super::components::{Enemy, EnemyKind, EnemyState, Shown};
use super::systems::{elapsed, face, go, player, rest, sees};
use super::{ARRIVED, centre};
use crate::features::audio::{self, components::Sound};
use crate::features::elements::components::BallBurn;
use crate::features::toys::components::{Ball, SmallBall};
use crate::game::GAMEPLAY;

/// How far from its home it roams on patrol.
pub const ROAM: f32 = 160.0;
/// Its patrol's share of its speed.
pub const PATROL_PACE: f32 = 0.4;
/// The reach of the burning balls it drifts to, and of those that heal it.
pub const FIRE_REACH: f32 = 400.0;
pub const HEAL_REACH: f32 = 96.0;
/// What each burning ball in reach heals a second, and the most a second.
pub const HEAL_EACH: f32 = 4.0;
pub const HEAL_MOST: f32 = 12.0;
/// Its health, its prefab's `max_health` stat and maximum: restated so a
/// save can move an older wisp's to it.
pub const MAX_HEALTH: f32 = 30.0;

/// Sets `wisp`'s flight toward `point` at `pace` of its speed, easing in
/// over its last 20 px; at rest with no point. Whether it is there.
fn fly(world: &mut World, wisp: Entity, point: Option<Vec2>, pace: f32) -> bool {
    let (Some(at), Some(point)) = (centre(world, wisp), point) else {
        rest(world, wisp);
        return true;
    };
    let delta = point - at;
    let distance = delta.length();
    let direction = if distance > 1.0 {
        delta / distance * pace * (distance / 20.0).min(1.0)
    } else {
        Vec2::ZERO
    };
    if let Some(mover) = world.get_mut::<TopDownMover>(wisp) {
        mover.set_direction(direction);
    }
    if direction.x.abs() > 0.05 {
        face(world, wisp, point.x);
    }
    distance <= ARRIVED
}

/// The nearest burning ball within [`FIRE_REACH`] of `at`.
fn nearest_fire(world: &World, at: Vec2) -> Option<Vec2> {
    burning(world)
        .filter(|ball| ball.distance(at) <= FIRE_REACH)
        .min_by(|a, b| a.distance(at).total_cmp(&b.distance(at)))
}

/// The burning balls' centres.
fn burning(world: &World) -> impl Iterator<Item = Vec2> + '_ {
    world
        .query::<(&Ball, &BallBurn, &Position, Option<&SmallBall>)>()
        .filter(|(_, burn, ..)| burn.remaining > 0.0)
        .map(|(_, _, at, small)| {
            let centre = if small.is_some() {
                crate::features::toys::SMALL_BALL_CENTRE
            } else {
                crate::features::toys::BALL_CENTRE
            };
            at.0 + centre
        })
}

/// On patrol it loops about its home, or drifts to a fire.
pub fn patrol(world: &mut World, wisp: Entity) {
    let Some(at) = centre(world, wisp) else {
        return;
    };
    if let Some(fire) = nearest_fire(world, at) {
        fly(world, wisp, Some(fire), PATROL_PACE);
        return;
    }
    let Some(home) = world.get::<Enemy>(wisp).map(Enemy::home) else {
        return;
    };
    let t = elapsed(world, wisp);
    let target = home + Vec2::new(0.6 * ROAM * (0.5 * t).sin(), 0.25 * ROAM * (0.9 * t).sin());
    fly(world, wisp, Some(target), PATROL_PACE);
}

/// The chase: straight at the player while it sees it, its touch the
/// attack; it searches once it loses sight.
pub fn chase(world: &mut World, wisp: Entity) {
    if !sees(world, wisp) {
        go(world, wisp, EnemyState::Search);
        return;
    }
    let target = player(world).map(|(_, at)| at);
    fly(world, wisp, target, 1.0);
}

/// Toward `point`, or a fire on the way; whether it is there.
pub fn seek(world: &mut World, wisp: Entity, point: Option<Vec2>) -> bool {
    if let Some(fire) = centre(world, wisp).and_then(|at| nearest_fire(world, at)) {
        fly(world, wisp, Some(fire), 0.6);
        return false;
    }
    fly(world, wisp, point, 0.6)
}

/// Its alert: the flame flares, with its sound.
pub fn flare(world: &mut World, wisp: Entity) {
    if let Some(at) = centre(world, wisp) {
        audio::play(world, Sound::WispFlare, at);
    }
}

/// `wisp_fire`, in the intent phase after the machine: each live wisp
/// outside a chase heals [`HEAL_EACH`] a second for each burning ball within
/// [`HEAL_REACH`], [`HEAL_MOST`] at most, through a heal request the step's
/// resolve takes.
pub fn fire(world: &mut World) {
    let dt = world.resource::<tungsten::core::Time>().delta();
    if dt <= 0.0 {
        return;
    }
    let wisps: Vec<(Entity, Vec2)> = world
        .query::<(Entity, &Enemy, &Health, &StateMachine<EnemyState>)>()
        .filter(|(_, enemy, health, machine)| {
            enemy.kind == EnemyKind::Wisp
                && !health.is_dead()
                && !machine.is_held()
                && !matches!(machine.state(), EnemyState::Chase | EnemyState::Alert)
        })
        .filter_map(|(entity, ..)| Some((entity, centre(world, entity)?)))
        .collect();
    for (wisp, at) in wisps {
        let near = burning(world)
            .filter(|ball| ball.distance(at) <= HEAL_REACH)
            .count();
        let rate = (HEAL_EACH * near as f32).min(HEAL_MOST);
        if rate > 0.0 {
            HealRequest::new(wisp, rate * dt).queue(world);
        }
    }
}

/// Its clip for `state`: the flare while alerted, else afloat.
#[must_use]
pub fn clip(state: EnemyState) -> &'static str {
    if state == EnemyState::Alert {
        "wisp_flare"
    } else {
        "wisp_float"
    }
}

/// A wisp's flare lights a second ember emitter at its centre, which goes
/// when the flare ends: more embers while it flares. Presentation.
pub fn flare_embers(world: &mut World, wisp: Entity) {
    let flaring = world
        .get::<Enemy>(wisp)
        .is_some_and(|enemy| enemy.kind == EnemyKind::Wisp)
        && world
            .get::<StateMachine<EnemyState>>(wisp)
            .is_some_and(|machine| machine.state() == EnemyState::Alert && !machine.is_held())
        && world
            .get::<Health>(wisp)
            .is_some_and(|health| !health.is_dead());
    let current = world.get::<Shown>(wisp).and_then(|shown| shown.flare);
    match (flaring, current) {
        (true, None) => {
            let Some(config) = world
                .get_resource::<ParticleConfigRegistry>()
                .and_then(|registry| registry.id_for_name("ex10_wisp_embers"))
            else {
                return;
            };
            let seed = 0x3a_0000 + u64::from(wisp.id());
            // A child, so it follows the wisp and goes with it.
            let emitter = spawn_child(
                world,
                wisp,
                EnemyKind::Wisp.info().centre,
                (
                    ParticleEmitter::with_seed(config, seed),
                    ParticleEmitterState::default(),
                    SceneEntity { state_id: GAMEPLAY },
                ),
            );
            if let Some(shown) = world.get_mut::<Shown>(wisp) {
                shown.flare = Some(emitter);
            }
        }
        (true, Some(_)) | (false, None) => {}
        (false, Some(emitter)) => {
            world.despawn(emitter);
            if let Some(shown) = world.get_mut::<Shown>(wisp) {
                shown.flare = None;
            }
        }
    }
}
