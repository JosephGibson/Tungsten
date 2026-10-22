//! The frost crawler: an alien, many-legged walker on the kit's platformer
//! controller. It walks its ledge, turning at the ends and at walls,
//! chills what it touches and, after a hissing tell, lunges. It never steps
//! into the staff light's drawn glow: it holds at its edge, facing the
//! player, scared, and lunges only at a player whose light is out.

use glam::Vec2;
use tungsten::core::{BodyKind, Entity, Position, RigidBody, Time, World};
use tungsten::kit::PlatformerController;
use tungsten::physics::{SpatialFilter, cast_ray};

use super::components::{Enemy, EnemyKind, EnemyState, Scared};
use super::systems::{elapsed, face, go, player, rest, sees};
use super::{ARRIVED, centre};
use crate::features::audio::{self, components::Sound};
use crate::features::level::components::Glow;
use crate::features::player::components::Lantern;
use crate::shared::spawn_burst;

/// Its patrol's and its search's share of its run speed.
pub const PATROL_PACE: f32 = 0.5;
pub const SEARCH_PACE: f32 = 0.7;
/// How near the player, across, its chase winds up a lunge.
pub const LUNGE_RANGE: f32 = 220.0;
/// The lunge's tell, its launch and how long its input stays locked.
pub const TELL_SECS: f32 = 0.4;
pub const LUNGE_SPEED: f32 = 520.0;
pub const LUNGE_LIFT: f32 = 380.0;
pub const LUNGE_LOCK: f32 = 0.6;
/// The longest a lunge runs before the recovery, landed or not.
pub const LUNGE_MOST: f32 = 1.0;
/// How long its scared look holds after the light last refused its walk,
/// in game seconds: a refusal after that starts a new scare, with its hiss
/// and its puff of frost breath (B4).
pub const SCARED_HOLD: f32 = 0.4;
pub const FROST_BREATH: &str = "ex10_frost_breath";

/// Whether a cast from `from` along `direction` meets ground within
/// `distance`: a solid tile or a static body, never a ball or the player.
fn solid(world: &World, from: Vec2, direction: Vec2, distance: f32, me: Entity) -> bool {
    let filter = SpatialFilter::new().excluding(me);
    cast_ray(world, from, direction, distance, &filter).is_some_and(|hit| {
        hit.entity.is_none_or(|entity| {
            world
                .get::<RigidBody>(entity)
                .is_some_and(|body| body.kind == BodyKind::Static)
        })
    })
}

/// Whether walking `crawler` along `heading` is blocked: a wall ahead, or
/// no ground under its leading feet.
fn blocked(world: &World, crawler: Entity, heading: f32) -> bool {
    let Some(at) = centre(world, crawler) else {
        return true;
    };
    let half = EnemyKind::Crawler.info().half;
    let wall = solid(world, at, Vec2::new(heading, 0.0), half.x + 8.0, crawler);
    let toes = Vec2::new(at.x + heading * (half.x + 6.0), at.y);
    let ledge = !solid(world, toes, Vec2::Y, half.y + 14.0, crawler);
    wall || ledge
}

/// The staff light's drawn glow, its centre and its reach, while the
/// player lives and the light burns: the glow on the flame of the frame
/// shown (I8).
#[must_use]
pub fn light(world: &World) -> Option<(Vec2, f32)> {
    let (player, _) = player(world)?;
    let lit = world.get::<Lantern>(player).is_none_or(Lantern::lit);
    let glow = world.get::<Glow>(player)?;
    let at = world.get::<Position>(player)?.0;
    lit.then(|| (glow.centre(at), glow.reach()))
}

/// Whether a step along `heading` takes `crawler` into the staff light.
fn into_light(world: &World, crawler: Entity, heading: f32) -> bool {
    let (Some((light, reach)), Some(at)) = (light(world), centre(world, crawler)) else {
        return false;
    };
    let next = at + Vec2::new(heading * 8.0, 0.0);
    next.distance(light) <= reach && next.distance(light) < at.distance(light)
}

/// Whether `crawler` shows its scare: the light refused its walk within
/// [`SCARED_HOLD`].
#[must_use]
pub fn scared(world: &World, crawler: Entity) -> bool {
    let now = world.get_resource::<Time>().map_or(0.0, Time::elapsed);
    world
        .get::<Scared>(crawler)
        .is_some_and(|scared| now - scared.at <= f64::from(SCARED_HOLD))
}

/// The light refused `crawler`'s walk: a scare starts, with a hiss and a
/// puff of frost breath at its mouth, unless one shows already.
fn scare(world: &mut World, crawler: Entity) {
    let fresh = !scared(world, crawler);
    let now = world.get_resource::<Time>().map_or(0.0, Time::elapsed);
    world.insert(crawler, Scared { at: now });
    let heading = world.get::<Enemy>(crawler).map_or(1.0, |mine| mine.heading);
    if let (true, Some(at)) = (fresh, centre(world, crawler)) {
        let mouth = at + Vec2::new(heading * 26.0, -2.0);
        spawn_burst(world, FROST_BREATH, mouth);
        audio::play(world, Sound::CrawlerScared, mouth);
    }
}

/// Walks `crawler` along `heading` at `pace` of its run speed, unless that
/// is blocked or steps into the light: then it stands, facing the player at
/// the light's edge, scared. Whether it walks.
fn walk(world: &mut World, crawler: Entity, heading: f32, pace: f32) -> bool {
    if into_light(world, crawler, heading) {
        rest(world, crawler);
        if let Some((_, at)) = player(world) {
            face(world, crawler, at.x);
        }
        scare(world, crawler);
        return false;
    }
    if blocked(world, crawler, heading) {
        rest(world, crawler);
        return false;
    }
    if let Some(mine) = world.get_mut::<Enemy>(crawler) {
        mine.heading = heading;
    }
    if let Some(controller) = world.get_mut::<PlatformerController>(crawler) {
        controller.set_run(heading * pace);
    }
    true
}

/// On patrol it walks its ledge, turning at each end and at walls.
pub fn patrol(world: &mut World, crawler: Entity) {
    let Some(heading) = world.get::<Enemy>(crawler).map(|mine| mine.heading) else {
        return;
    };
    let grounded = world
        .get::<PlatformerController>(crawler)
        .is_some_and(PlatformerController::grounded);
    if !grounded {
        rest(world, crawler);
        return;
    }
    if !walk(world, crawler, heading, PATROL_PACE)
        && !into_light(world, crawler, heading)
        && !walk(world, crawler, -heading, PATROL_PACE)
    {
        rest(world, crawler);
    }
}

/// The chase: toward the player while it sees it, winding up a lunge once
/// within [`LUNGE_RANGE`] across, unless the staff light burns, which it
/// keeps out of; it searches once it loses sight.
pub fn chase(world: &mut World, crawler: Entity) {
    if !sees(world, crawler) {
        go(world, crawler, EnemyState::Search);
        return;
    }
    let (Some((_, target)), Some(at)) = (player(world), centre(world, crawler)) else {
        return;
    };
    let grounded = world
        .get::<PlatformerController>(crawler)
        .is_some_and(PlatformerController::grounded);
    let across = target.x - at.x;
    let lit = light(world).is_some();
    if grounded
        && !lit
        && across.abs() <= LUNGE_RANGE
        && (target.y - at.y).abs() <= 2.0 * LUNGE_RANGE
    {
        face(world, crawler, target.x);
        go(world, crawler, EnemyState::WindUp);
        rest(world, crawler);
        return;
    }
    walk(world, crawler, across.signum(), 1.0);
}

/// Its tell: it rears and hisses.
pub fn enter_windup(world: &mut World, crawler: Entity) {
    if let Some((_, target)) = player(world) {
        face(world, crawler, target.x);
    }
    if let Some(at) = centre(world, crawler) {
        audio::play(world, Sound::CrawlerHiss, at);
    }
}

/// The tell lasts [`TELL_SECS`], then the lunge.
pub fn windup(world: &mut World, crawler: Entity) {
    rest(world, crawler);
    if elapsed(world, crawler) >= TELL_SECS - 1e-4 {
        go(world, crawler, EnemyState::Attack);
    }
}

/// The lunge: a leap at the player at [`LUNGE_SPEED`], its input locked
/// while it flies; its touch is what hurts.
pub fn enter_attack(world: &mut World, crawler: Entity) {
    let Some(heading) = world.get::<Enemy>(crawler).map(|mine| mine.heading) else {
        return;
    };
    if let Some(controller) = world.get_mut::<PlatformerController>(crawler) {
        controller.launch(Vec2::new(heading * LUNGE_SPEED, -LUNGE_LIFT));
        controller.lock_input(LUNGE_LOCK);
    }
    if let Some(at) = centre(world, crawler) {
        audio::play(world, Sound::CrawlerLunge, at);
    }
}

/// The lunge ends when it lands, or after [`LUNGE_MOST`].
pub fn attack(world: &mut World, crawler: Entity) {
    let t = elapsed(world, crawler);
    let grounded = world
        .get::<PlatformerController>(crawler)
        .is_some_and(PlatformerController::grounded);
    if (t > 0.15 && grounded) || t >= LUNGE_MOST {
        go(world, crawler, EnemyState::Recover);
    }
}

/// Walks toward `point`'s column: its search's or its home's. Whether it is
/// there, or can go no nearer.
pub fn seek(world: &mut World, crawler: Entity, point: Option<Vec2>) -> bool {
    let (Some(point), Some(at)) = (point, centre(world, crawler)) else {
        rest(world, crawler);
        return true;
    };
    let across = point.x - at.x;
    if across.abs() <= ARRIVED {
        rest(world, crawler);
        return true;
    }
    !walk(world, crawler, across.signum(), SEARCH_PACE)
        && !into_light(world, crawler, across.signum())
}

/// Its clip for `state`: the lunge's through its tell and its leap, else
/// its scare while it shows one, else its walk.
#[must_use]
pub fn clip(state: EnemyState, scared: bool) -> &'static str {
    if matches!(state, EnemyState::WindUp | EnemyState::Attack) {
        "crawler_lunge"
    } else if scared {
        "crawler_scared"
    } else {
        "crawler_walk"
    }
}
