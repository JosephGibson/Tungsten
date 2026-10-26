//! The frost crawler: an alien, many-legged walker on the kit's platformer
//! controller. It walks its ledge, turning at the ends and at walls,
//! chills what it touches and, after a hissing tell, lunges. It fears the
//! staff light past its drawn glow, to [`FEAR_SHARE`] of its reach: it
//! never steps over that edge, holding there facing the player, scared;
//! caught inside it, it backs out ([`retreat`]); it lunges only at a player
//! whose light is out. Chasing, searching or going home it hops bodies and
//! steps and leaps gaps by its [`HOPS`].

use glam::Vec2;
use tungsten::core::{Entity, Position, Time, Velocity, World};
use tungsten::kit::PlatformerController;

use super::components::{Brain, Enemy, EnemyState, Scared};
use super::hop::{self, Ahead, Hops, Leap, grounded};
use super::systems::{blocked, elapsed, face, go, player, rest, sees, stride};
use super::{ARRIVED, centre};
use crate::features::audio::{self, components::Sound};
use crate::features::level::components::Glow;
use crate::features::player::components::Lantern;
use crate::shared::spawn_burst;

/// Its patrol's and its search's share of its run speed.
pub const PATROL_PACE: f32 = 0.5;
pub const SEARCH_PACE: f32 = 0.7;
/// How near the player, across, its chase winds up a lunge.
pub const LUNGE_RANGE: f32 = 300.0;
/// How far above its centre the player's may be for a lunge: the leap
/// rises about 53 px, so at a player higher up it would only hop into the
/// step below them.
pub const LUNGE_RISE: f32 = 64.0;
/// The lunge's tell, its launch and how long its input stays locked. At
/// the level's gravity the leap flies 0.34 s, 53 px up and 220 px across;
/// the lock holds its speed 0.1 s past the landing, about 70 px more, so
/// the lunge carries about 290 px, within its range.
pub const TELL_SECS: f32 = 0.4;
pub const LUNGE_SPEED: f32 = 640.0;
pub const LUNGE_LIFT: f32 = 620.0;
pub const LUNGE_LOCK: f32 = 0.45;
/// The longest a lunge runs before the recovery, landed or not.
pub const LUNGE_MOST: f32 = 1.0;
/// The cold its touch deals, its prefab's `damage` stat and contact
/// damage, and its run, its `move_speed` stat and its controller's run
/// speed: restated so a save can move an older crawler's bases to them.
pub const DAMAGE: f32 = 15.0;
pub const MOVE_SPEED: f32 = 210.0;
/// How long its scared look holds after the light last refused its walk,
/// in game seconds: a refusal after that starts a new scare, with its hiss
/// and its puff of frost breath (B4).
pub const SCARED_HOLD: f32 = 0.4;
pub const FROST_BREATH: &str = "ex10_frost_breath";
/// Its fear's reach over the staff light's drawn one (V20): 227.5 px for
/// the staff's 175, the drawn light unchanged.
pub const FEAR_SHARE: f32 = 1.3;
/// Its retreat's share of its run speed as it backs out of the fear.
pub const RETREAT_PACE: f32 = 0.8;
/// Its hops (V21): a hop of 720 px/s up over a body or a step no higher
/// than 64 px, a step down a drop no deeper than 96 px, and a leap over a
/// gap of 420 px/s across and 600 up, its input locked 0.35 s, which rises
/// 50 px and carries it about 140 px to level ground.
pub const HOPS: Hops = Hops {
    rise: 720.0,
    clear: 64.0,
    step_down: 96.0,
    leap: Some(Leap {
        across: 420.0,
        lift: 600.0,
        lock: 0.35,
    }),
};

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

/// The edge of its fear while the light burns: the light's centre and
/// [`FEAR_SHARE`] of its reach.
#[must_use]
pub fn fear(world: &World) -> Option<(Vec2, f32)> {
    light(world).map(|(centre, reach)| (centre, FEAR_SHARE * reach))
}

/// Whether `crawler`'s centre lies inside its fear's edge.
#[must_use]
pub fn afraid(world: &World, crawler: Entity) -> bool {
    match (fear(world), centre(world, crawler)) {
        (Some((light, reach)), Some(at)) => at.distance(light) < reach,
        _ => false,
    }
}

/// Whether a step along `heading` takes `crawler` over its fear's edge.
fn into_light(world: &World, crawler: Entity, heading: f32) -> bool {
    let (Some((light, reach)), Some(at)) = (fear(world), centre(world, crawler)) else {
        return false;
    };
    let next = at + Vec2::new(heading * 8.0, 0.0);
    next.distance(light) <= reach && next.distance(light) < at.distance(light)
}

/// Whether `crawler` shows its scare: the light refused its walk, or drove
/// it back, within [`SCARED_HOLD`].
#[must_use]
pub fn scared(world: &World, crawler: Entity) -> bool {
    let now = world.get_resource::<Time>().map_or(0.0, Time::elapsed);
    world
        .get::<Scared>(crawler)
        .is_some_and(|scared| now - scared.at <= f64::from(SCARED_HOLD))
}

/// The light refused `crawler`'s walk, or, `retreating`, drives it back: a
/// scare starts, with a puff of frost breath at its mouth and its hiss, or
/// its retreat's longer one, unless one shows already.
fn scare(world: &mut World, crawler: Entity, retreating: bool) {
    let fresh = !scared(world, crawler);
    let now = world.get_resource::<Time>().map_or(0.0, Time::elapsed);
    world.insert(crawler, Scared { at: now });
    let heading = world.get::<Enemy>(crawler).map_or(1.0, |mine| mine.heading);
    if let (true, Some(at)) = (fresh, centre(world, crawler)) {
        let mouth = at + Vec2::new(heading * 26.0, -2.0);
        spawn_burst(world, FROST_BREATH, mouth);
        let sound = if retreating {
            Sound::CrawlerRetreated
        } else {
            Sound::CrawlerScared
        };
        audio::play(world, sound, mouth);
    }
}

/// Stands `crawler` at its fear's edge, facing the player, scared.
fn hold(world: &mut World, crawler: Entity) {
    rest(world, crawler);
    if let Some((_, at)) = player(world) {
        face(world, crawler, at.x);
    }
    scare(world, crawler, false);
}

/// Backs `crawler` out of its fear (V20): facing the player, it runs
/// straight away from the light's centre at [`RETREAT_PACE`] of its run;
/// stopped behind by a wall or a ledge, it stands, cornered, its touch
/// hurting as ever. Its scare shows throughout, its first step sounding
/// [`Sound::CrawlerRetreated`] with a puff of frost breath; in the air it
/// keeps its run.
pub fn retreat(world: &mut World, crawler: Entity) {
    let (Some((light, _)), Some(at)) = (fear(world), centre(world, crawler)) else {
        return;
    };
    if let Some((_, target)) = player(world) {
        face(world, crawler, target.x);
    }
    scare(world, crawler, true);
    if !grounded(world, crawler) {
        return;
    }
    let heading = world.get::<Enemy>(crawler).map_or(1.0, |mine| mine.heading);
    let away = if (at.x - light.x).abs() > 1.0 {
        (at.x - light.x).signum()
    } else {
        -heading
    };
    if blocked(world, crawler, away) {
        rest(world, crawler);
    } else if let Some(controller) = world.get_mut::<PlatformerController>(crawler) {
        controller.set_run(away * RETREAT_PACE);
    }
}

/// Walks `crawler` along `heading` at `pace` of its run speed, unless that
/// is blocked or steps over its fear's edge: then it stands, held at the
/// edge. Whether it walks. Its patrol's walk: it keeps its ledge.
fn walk(world: &mut World, crawler: Entity, heading: f32, pace: f32) -> bool {
    if into_light(world, crawler, heading) {
        hold(world, crawler);
        return false;
    }
    stride(world, crawler, heading, pace)
}

/// Goes along `heading` at `pace` in its chase, its search or its way
/// home: held at its fear's edge as its walk is, else hopping, leaping and
/// stepping down what it can by its [`HOPS`], chittering at each hop or
/// leap. Whether it moves.
fn scuttle(world: &mut World, crawler: Entity, heading: f32, pace: f32) -> bool {
    if into_light(world, crawler, heading) {
        hold(world, crawler);
        return false;
    }
    let went = hop::scurry(world, crawler, heading, pace, &HOPS);
    if matches!(went, Ahead::Hop | Ahead::Leap)
        && let Some(at) = centre(world, crawler)
    {
        audio::play(world, Sound::CrawlerHopped, at);
    }
    went.moves()
}

/// On patrol it walks its ledge, turning at each end, at walls and, on a
/// leash, once that far from its home; afraid, it backs out first.
pub fn patrol(world: &mut World, crawler: Entity) {
    if afraid(world, crawler) {
        retreat(world, crawler);
        return;
    }
    let Some(mine) = world.get::<Enemy>(crawler).copied() else {
        return;
    };
    let mut heading = mine.heading;
    if let (Some(leash), Some(at)) = (mine.leash, centre(world, crawler)) {
        let out = at.x - mine.home().x;
        if out.abs() >= leash && out.signum() == heading {
            heading = -heading;
        }
    }
    if !grounded(world, crawler) {
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

/// The chase: afraid, it backs out first; else toward the player while it
/// sees it, hopping and leaping on its way, winding up a lunge once within
/// [`LUNGE_RANGE`] across and no more than [`LUNGE_RISE`] above it, unless
/// the staff light burns, whose fear's edge it keeps behind; it searches
/// once it loses sight. In the air it keeps its run.
pub fn chase(world: &mut World, crawler: Entity) {
    if afraid(world, crawler) {
        retreat(world, crawler);
        return;
    }
    if !sees(world, crawler) {
        go(world, crawler, EnemyState::Search);
        return;
    }
    let (Some((_, target)), Some(at)) = (player(world), centre(world, crawler)) else {
        return;
    };
    if !grounded(world, crawler) {
        return;
    }
    let across = target.x - at.x;
    let lit = light(world).is_some();
    let rise = at.y - target.y;
    if !lit && across.abs() <= LUNGE_RANGE && rise <= LUNGE_RISE && -rise <= 2.0 * LUNGE_RANGE {
        face(world, crawler, target.x);
        go(world, crawler, EnemyState::WindUp);
        rest(world, crawler);
        return;
    }
    scuttle(world, crawler, across.signum(), 1.0);
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

/// The tell lasts [`TELL_SECS`], then the lunge; afraid, it spends its
/// strike and chases again, so it backs out.
pub fn windup(world: &mut World, crawler: Entity) {
    rest(world, crawler);
    if afraid(world, crawler) {
        if let Some(brain) = world.get_mut::<Brain>(crawler) {
            brain.strike = None;
        }
        go(world, crawler, EnemyState::Chase);
        return;
    }
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
    if (t > 0.15 && grounded(world, crawler)) || t >= LUNGE_MOST {
        go(world, crawler, EnemyState::Recover);
    }
}

/// Goes toward `point`'s column, hopping and leaping on its way: its
/// search's or its home's; afraid, it backs out first. Whether it is there,
/// or can go no nearer.
pub fn seek(world: &mut World, crawler: Entity, point: Option<Vec2>) -> bool {
    if afraid(world, crawler) {
        retreat(world, crawler);
        return false;
    }
    let (Some(point), Some(at)) = (point, centre(world, crawler)) else {
        rest(world, crawler);
        return true;
    };
    let across = point.x - at.x;
    if across.abs() <= ARRIVED {
        rest(world, crawler);
        return true;
    }
    if !grounded(world, crawler) {
        return false;
    }
    !scuttle(world, crawler, across.signum(), SEARCH_PACE)
        && !into_light(world, crawler, across.signum())
}

/// Its clip for `state`: the lunge's three, its coil through the tell, its
/// stretch through the leap and its landing through the recovery; a hop in
/// the air; its retreat while it backs out of its fear, its scare while it
/// holds at the edge or stands cornered; else its walk.
#[must_use]
pub fn clip(world: &World, crawler: Entity, state: EnemyState) -> &'static str {
    let moving = world
        .get::<Velocity>(crawler)
        .is_some_and(|velocity| velocity.0.x.abs() > 1.0);
    match state {
        EnemyState::WindUp => "crawler_windup",
        EnemyState::Attack => "crawler_lunge",
        EnemyState::Recover => "crawler_recover",
        _ if !grounded(world, crawler) => "crawler_hop",
        _ if moving && afraid(world, crawler) => "crawler_retreat",
        _ if scared(world, crawler) => "crawler_scared",
        _ => "crawler_walk",
    }
}
