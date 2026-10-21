//! The rust beetle: a horned, shelled walker on the kit's platformer
//! controller. It walks its ground, turning at ledges and walls; once it
//! sees the player within [`CHARGE_RANGE`] across it paws the ground for
//! its tell, then charges along its heading at [`CHARGE_SPEED`], its
//! `beetle_charge` ability arming its hitbox, which hurts whatever it
//! meets, enemies too. Its shell is its front, the half its heading faces:
//! there a live beetle turns bullets as iron does, while its rear half
//! takes every hit 01 builds whole (`combat::Hit`). Iron that drives into
//! it at the crush speed kills it.

use glam::Vec2;
use tungsten::core::{Entity, World};
use tungsten::kit::{PlatformerController, use_ability};

use super::components::{Enemy, EnemyKind, EnemyState};
use super::systems::{elapsed, face, go, pace, player, rest, sees, stride};
use super::{ARRIVED, centre};
use crate::features::audio::{self, components::Sound};

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

/// Whether `entity` is a beetle.
#[must_use]
pub fn is_beetle(world: &World, entity: Entity) -> bool {
    world
        .get::<Enemy>(entity)
        .is_some_and(|enemy| enemy.kind == EnemyKind::Beetle)
}

/// Whether `point` lies on `beetle`'s front half: past its centre on the
/// side its heading faces.
#[must_use]
pub fn front(world: &World, beetle: Entity, point: Vec2) -> bool {
    let (Some(mine), Some(at)) = (world.get::<Enemy>(beetle), centre(world, beetle)) else {
        return false;
    };
    (point.x - at.x) * mine.heading > 0.0
}

/// Whether `target` is a beetle and `point` lies on its rear half, which
/// takes a hit whole.
#[must_use]
pub fn rear(world: &World, target: Entity, point: Vec2) -> bool {
    is_beetle(world, target) && !front(world, target, point)
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

/// The charge ends after [`CHARGE_SECS`].
pub fn attack(world: &mut World, beetle: Entity) {
    if elapsed(world, beetle) >= CHARGE_SECS - 1e-4 {
        go(world, beetle, EnemyState::Recover);
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
