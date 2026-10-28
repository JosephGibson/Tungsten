//! The cave bat: a near-black body on dark purple wings, flying on the
//! kit's top-down mover. It roosts hanging under the ceiling above where it
//! was placed; once it sees the player it flies at it, and within
//! [`DIVE_RANGE`] it flashes white for its tell, then dives at where the
//! player was at [`DIVE_SPEED`], its `bat_dive` ability arming its hitbox,
//! and climbs back to its roost. It burns and bleeds; frozen, it falls on a
//! borrowed controller and shatters when it lands.

use glam::Vec2;
use tungsten::core::{Entity, World};
use tungsten::kit::{Health, Modifier, PlatformerController, Stats, TopDownMover, use_ability};
use tungsten::physics::{SpatialFilter, cast_ray};

use super::components::{Enemy, EnemyKind, EnemyState};
use super::systems::{elapsed, face, go, player, rest, sees};
use super::{ARRIVED, WALL_LAYERS, centre};
use crate::features::audio::{self, components::Sound};
use crate::features::combat::{Hit, Kind};

/// How far above its spawn a ceiling makes its roost.
pub const ROOST_REACH: f32 = 480.0;
/// Its patrol's and its search's share of its speed.
pub const PATROL_PACE: f32 = 0.6;
pub const SEARCH_PACE: f32 = 0.8;
/// How near the player its chase starts its tell, and how far above the
/// player it flies to get there.
pub const DIVE_RANGE: f32 = 300.0;
pub const DIVE_ABOVE: f32 = 120.0;
/// The white flash before the dive, the dive's speed and its seconds,
/// which its ability's hitbox window matches.
pub const FLASH_SECS: f32 = 0.5;
pub const DIVE_SPEED: f32 = 700.0;
pub const DIVE_SECS: f32 = 0.6;
/// Its dive's ability, which arms its hitbox, and the source of the dive's
/// speed on its `move_speed`.
pub const DIVE: &str = "bat_dive";
pub const DIVING: &str = "dive";

/// Where a bat placed with its body's centre at `at` hangs, as its body's
/// centre: its frame's top, where its feet grip, on the first ceiling within
/// [`ROOST_REACH`] above, a solid tile or a wall (a slab, iron or a wooden
/// deck); with none, `at`.
#[must_use]
pub fn roost(world: &World, at: Vec2) -> Vec2 {
    let grip = super::body(world, EnemyKind::Bat).centre.y;
    let tiles = SpatialFilter::new().with_bodies(false);
    let walls = SpatialFilter::new().with_mask(WALL_LAYERS);
    [tiles, walls]
        .iter()
        .filter_map(|filter| cast_ray(world, at, -Vec2::Y, ROOST_REACH, filter))
        .map(|hit| hit.distance)
        .min_by(f32::total_cmp)
        .map_or(at, |distance| at - Vec2::new(0.0, distance - grip))
}

/// Sets `bat`'s flight toward `point` at `pace` of its speed, easing in
/// over its last 20 px; at rest with no point. Whether it is there.
fn fly(world: &mut World, bat: Entity, point: Option<Vec2>, pace: f32) -> bool {
    let (Some(at), Some(point)) = (centre(world, bat), point) else {
        rest(world, bat);
        return true;
    };
    let delta = point - at;
    let distance = delta.length();
    let direction = if distance > 1.0 {
        delta / distance * pace * (distance / 20.0).min(1.0)
    } else {
        Vec2::ZERO
    };
    if let Some(mover) = world.get_mut::<TopDownMover>(bat) {
        mover.set_direction(direction);
    }
    if direction.x.abs() > 0.05 {
        face(world, bat, point.x);
    }
    distance <= ARRIVED
}

/// Whether `bat` hangs at its roost.
fn roosting(world: &World, bat: Entity) -> bool {
    let (Some(home), Some(at)) = (world.get::<Enemy>(bat).map(Enemy::home), centre(world, bat))
    else {
        return false;
    };
    at.distance(home) <= ARRIVED
}

/// On patrol it hangs at its roost, flying back there when it is not; it
/// chases again while it sees the player, as after a dive.
pub fn patrol(world: &mut World, bat: Entity) {
    if sees(world, bat) {
        go(world, bat, EnemyState::Chase);
        return;
    }
    let home = world.get::<Enemy>(bat).map(Enemy::home);
    fly(world, bat, home, PATROL_PACE);
}

/// The chase: toward a point above the player while it sees it, its tell
/// once within [`DIVE_RANGE`]; it searches once it loses sight.
pub fn chase(world: &mut World, bat: Entity) {
    if !sees(world, bat) {
        go(world, bat, EnemyState::Search);
        return;
    }
    let (Some((_, target)), Some(at)) = (player(world), centre(world, bat)) else {
        return;
    };
    if at.distance(target) <= DIVE_RANGE {
        rest(world, bat);
        face(world, bat, target.x);
        go(world, bat, EnemyState::WindUp);
        return;
    }
    fly(world, bat, Some(target - Vec2::new(0.0, DIVE_ABOVE)), 1.0);
}

/// Its tell: it faces the player, flashes white and screeches.
pub fn enter_windup(world: &mut World, bat: Entity) {
    if let Some((_, target)) = player(world) {
        face(world, bat, target.x);
    }
    if let Some(at) = centre(world, bat) {
        audio::play(world, Sound::BatScreech, at);
    }
}

/// The flash lasts [`FLASH_SECS`], then the dive.
pub fn windup(world: &mut World, bat: Entity) {
    rest(world, bat);
    if elapsed(world, bat) >= FLASH_SECS - 1e-4 {
        go(world, bat, EnemyState::Attack);
    }
}

/// The dive: straight at where the player is at [`DIVE_SPEED`], its hitbox
/// armed for [`DIVE_SECS`] by its ability; with no player, it climbs back.
pub fn enter_attack(world: &mut World, bat: Entity) {
    let (Some(at), Some((_, target))) = (centre(world, bat), player(world)) else {
        go(world, bat, EnemyState::Recover);
        return;
    };
    if let Some(stats) = world.get_mut::<Stats>(bat) {
        stats.remove_modifiers(DIVING);
        stats.add_modifier(Stats::MOVE_SPEED, DIVING, Modifier::Override(DIVE_SPEED));
    }
    if let Some(mover) = world.get_mut::<TopDownMover>(bat) {
        mover.set_direction((target - at).normalize_or(Vec2::Y));
    }
    face(world, bat, target.x);
    use_ability(world, bat, DIVE);
    audio::play(world, Sound::BatDive, at);
}

/// The dive lasts [`DIVE_SECS`], then the climb back.
pub fn attack(world: &mut World, bat: Entity) {
    if elapsed(world, bat) >= DIVE_SECS - 1e-4 {
        end_dive(world, bat);
        go(world, bat, EnemyState::Recover);
    }
}

/// Its own speed back.
fn end_dive(world: &mut World, bat: Entity) {
    if let Some(stats) = world.get_mut::<Stats>(bat) {
        stats.remove_modifiers(DIVING);
    }
}

/// After a dive it climbs back to its roost, then roosts.
pub fn recover(world: &mut World, bat: Entity) {
    end_dive(world, bat);
    let home = world.get::<Enemy>(bat).map(Enemy::home);
    if fly(world, bat, home, 1.0) {
        go(world, bat, EnemyState::Patrol);
    }
}

/// Toward `point`, its search's or its home's; whether it is there.
pub fn seek(world: &mut World, bat: Entity, point: Option<Vec2>) -> bool {
    fly(world, bat, point, SEARCH_PACE)
}

/// A frozen bat on its borrowed controller shatters once it lands: a hit of
/// all its health, so it ends in the ice's burst.
pub fn land(world: &mut World, bat: Entity) {
    let grounded = world
        .get::<PlatformerController>(bat)
        .is_some_and(PlatformerController::grounded);
    let Some(left) = world
        .get::<Health>(bat)
        .filter(|health| !health.is_dead())
        .map(Health::current)
    else {
        return;
    };
    if let (true, Some(at)) = (grounded, centre(world, bat)) {
        Hit::new(bat, [(Kind::Physical, left)]).at(at).queue(world);
    }
}

/// Its clip for `state`: the flash through its tell, the dive through its
/// attack, hanging at its roost on patrol, else in flight.
#[must_use]
pub fn clip(world: &World, bat: Entity, state: EnemyState) -> &'static str {
    match state {
        EnemyState::WindUp => "bat_windup",
        EnemyState::Attack => "bat_dive",
        EnemyState::Patrol if roosting(world, bat) => "bat_roost",
        _ => "bat_fly",
    }
}
