//! The cave bat: a near-black body on dark purple wings, flying on the
//! kit's top-down mover. It roosts hanging under the ceiling above where it
//! was placed; once it sees the player it flies at it, and within
//! [`DIVE_RANGE`] it flashes white for its tell, then dives at where the
//! player was, its `bat_dive` ability giving it its `bat_diving` speed and
//! arming its hitbox, until the hitbox's window ends, and climbs back to
//! its roost. It burns and bleeds; frozen, it falls on a
//! borrowed controller and shatters when it lands.

use glam::Vec2;
use tungsten::core::{DataRef, Entity, World};
use tungsten::kit::{Health, PlatformerController, TopDownMover, end_effect, use_ability};
use tungsten::physics::{SpatialFilter, cast_ray};

use super::components::{Enemy, EnemyKind, EnemyState};
use super::systems::{armed, elapsed, face, go, player, rest, sees};
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
/// The white flash before the dive.
pub const FLASH_SECS: f32 = 0.5;
/// Its dive's ability, which gives it the dive's speed, [`DIVE_EFFECT`]'s
/// override on its `move_speed`, and arms its hitbox for the dive's window.
pub const DIVE: &str = "bat_dive";
pub const DIVE_EFFECT: &str = "bat_diving";

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
pub fn roosting(world: &World, bat: Entity) -> bool {
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

/// The dive: straight at where the player is, at the speed its ability's
/// [`DIVE_EFFECT`] effect gives it, its hitbox armed for the dive's window by
/// the same ability; with no player, it climbs back.
pub fn enter_attack(world: &mut World, bat: Entity) {
    let (Some(at), Some((_, target))) = (centre(world, bat), player(world)) else {
        go(world, bat, EnemyState::Recover);
        return;
    };
    if let Some(mover) = world.get_mut::<TopDownMover>(bat) {
        mover.set_direction((target - at).normalize_or(Vec2::Y));
    }
    face(world, bat, target.x);
    use_ability(world, bat, DIVE);
    audio::play(world, Sound::BatDive, at);
}

/// The dive lasts its hitbox's window, then the climb back.
pub fn attack(world: &mut World, bat: Entity) {
    if !armed(world, bat) {
        go(world, bat, EnemyState::Recover);
    }
}

/// After a dive it climbs back to its roost, then roosts, at its own
/// speed: the dive's effect ends, should it run still.
pub fn recover(world: &mut World, bat: Entity) {
    end_effect(world, bat, &DataRef::by_id(DIVE_EFFECT));
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
