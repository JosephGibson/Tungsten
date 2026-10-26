//! The enemies' systems: the state machine and the hooks every kind
//! shares, the sight, the alerts, the hold, the deaths and the clips.

use glam::Vec2;
use tungsten::core::{
    AnimationRegistry, AnimationState, BodyKind, Entity, EventCursor, EventQueue, Position,
    RigidBody, Sprite, Time, Transform, Visibility, World,
};
use tungsten::kit::{
    ContactDamage, Damaged, Health, Hitbox, PlatformerController, StateMachine, StateMachinePlugin,
    TopDownMover,
};
use tungsten::physics::{SpatialFilter, cast_ray};

use super::components::{
    Brain, Displaced, Enemy, EnemyKind, EnemyState, HeldTouch, Shown, Sight, Spotted,
};
use super::{
    ALERT_MARK_SECS, ALERT_REACH, DORMANT_PAST, ENEMY_STATES, SEARCH_SECS, SIGHT_EVERY, WALK_STEP,
    WALL_LAYERS, bat, beetle, centre, crawler, goblin, troll, wisp,
};
use crate::features::audio::{self, components::Sound};
use crate::features::combat::{BURNING, FROZEN, STUNNED, has_status};
use crate::features::dummy::components::Dummy;
use crate::features::elements;
use crate::features::props::components::{Crate, Keg};
use crate::features::run::Run;
use crate::features::toys::components::{IronBlock, IronScrap};
use crate::shared::spawn_key_burst;

/// The machine every enemy thinks on: each state's hooks run its kind's
/// rules.
#[must_use]
pub fn machine() -> StateMachinePlugin<EnemyState> {
    StateMachinePlugin::new(ENEMY_STATES)
        .on_update(EnemyState::Patrol, patrol)
        .on_enter(EnemyState::Alert, enter_alert)
        .on_update(EnemyState::Alert, alert)
        .on_update(EnemyState::Chase, chase)
        .on_enter(EnemyState::WindUp, enter_windup)
        .on_update(EnemyState::WindUp, windup)
        .on_enter(EnemyState::Attack, enter_attack)
        .on_update(EnemyState::Attack, attack)
        .on_update(EnemyState::Recover, recover)
        .on_enter(EnemyState::Search, enter_search)
        .on_update(EnemyState::Search, search)
        .on_update(EnemyState::Return, home)
}

/// `enemy`'s kind.
fn kind(world: &World, enemy: Entity) -> Option<EnemyKind> {
    world.get::<Enemy>(enemy).map(|enemy| enemy.kind)
}

/// `enemy`'s machine.
pub fn machine_of(world: &mut World, enemy: Entity) -> Option<&mut StateMachine<EnemyState>> {
    world.get_mut::<StateMachine<EnemyState>>(enemy)
}

/// The seconds `enemy` has spent in its state.
#[must_use]
pub fn elapsed(world: &World, enemy: Entity) -> f32 {
    world
        .get::<StateMachine<EnemyState>>(enemy)
        .map_or(0.0, StateMachine::elapsed)
}

/// Asks `enemy`'s machine for `state` at its next visit.
pub fn go(world: &mut World, enemy: Entity, state: EnemyState) {
    if let Some(machine) = machine_of(world, enemy) {
        machine.go(state);
    }
}

/// The run's player's body centre, while it lives.
#[must_use]
pub fn player(world: &World) -> Option<(Entity, Vec2)> {
    let player = world.get_resource::<Run>()?.player;
    if world.get::<Health>(player).is_none_or(Health::is_dead) {
        return None;
    }
    Some((player, crate::features::player::centre(world, player)?))
}

/// Whether `enemy` sees the player at its last check.
#[must_use]
pub fn sees(world: &World, enemy: Entity) -> bool {
    world.get::<Sight>(enemy).is_some_and(|sight| sight.sees)
}

/// Turns `enemy` to face `x`.
pub fn face(world: &mut World, enemy: Entity, x: f32) {
    let Some(at) = centre(world, enemy) else {
        return;
    };
    if let Some(mine) = world.get_mut::<Enemy>(enemy)
        && (x - at.x).abs() > 1.0
    {
        mine.heading = (x - at.x).signum();
    }
}

/// Stops `enemy`'s mover: a walker's run at rest, a flyer's direction at
/// rest.
pub fn rest(world: &mut World, enemy: Entity) {
    if let Some(controller) = world.get_mut::<PlatformerController>(enemy) {
        controller.set_run(0.0);
    }
    if let Some(mover) = world.get_mut::<TopDownMover>(enemy) {
        mover.set_direction(Vec2::ZERO);
    }
}

/// The most dynamic bodies [`solid`]'s ray looks past.
const PAST_BODIES: usize = 4;

/// Whether a cast from `from` along `direction` meets ground within
/// `distance`: a solid tile or a static body, never an enemy or the player.
/// It looks past a ball, a rock, a crate or any dynamic body, its ray
/// going on from inside the body it met, which it then passes, so a body
/// hides no wall or ground behind it.
pub(crate) fn solid(world: &World, from: Vec2, direction: Vec2, distance: f32, me: Entity) -> bool {
    let filter = SpatialFilter::new().excluding(me);
    let (mut from, mut left) = (from, distance);
    for _ in 0..=PAST_BODIES {
        let Some(hit) = cast_ray(world, from, direction, left, &filter) else {
            return false;
        };
        let Some(entity) = hit.entity else {
            return true;
        };
        match world.get::<RigidBody>(entity).map(|body| body.kind) {
            Some(BodyKind::Static) => return true,
            Some(BodyKind::Dynamic) => {
                // A hair inside its edge, where the ray passes it.
                let into = hit.distance + 0.01;
                from += direction * into;
                left -= into;
                if left <= 0.0 {
                    return false;
                }
            }
            _ => return false,
        }
    }
    false
}

/// Whether walking `walker` along `heading` is blocked: a wall ahead, or no
/// ground under its leading feet within [`WALK_STEP`] past a step's depth,
/// so a walker on a ball walks on, off it, while a tile's ledge turns it.
#[must_use]
pub fn blocked(world: &World, walker: Entity, heading: f32) -> bool {
    let (Some(at), Some(kind)) = (centre(world, walker), kind(world, walker)) else {
        return true;
    };
    let half = kind.info().half;
    let wall = solid(world, at, Vec2::new(heading, 0.0), half.x + 8.0, walker);
    let toes = Vec2::new(at.x + heading * (half.x + 6.0), at.y);
    let ledge = !solid(world, toes, Vec2::Y, half.y + 14.0 + WALK_STEP, walker);
    wall || ledge
}

/// Walks `walker` along `heading` at `pace` of its run speed, unless that is
/// blocked: then it stands. Whether it walks.
pub fn stride(world: &mut World, walker: Entity, heading: f32, pace: f32) -> bool {
    if blocked(world, walker, heading) {
        rest(world, walker);
        return false;
    }
    if let Some(mine) = world.get_mut::<Enemy>(walker) {
        mine.heading = heading;
    }
    if let Some(controller) = world.get_mut::<PlatformerController>(walker) {
        controller.set_run(heading * pace);
    }
    true
}

/// Walks `walker` its ground at `pace`, turning at each end and at walls;
/// in the air it waits to land.
pub fn pace(world: &mut World, walker: Entity, pace: f32) {
    let Some(heading) = world.get::<Enemy>(walker).map(|mine| mine.heading) else {
        return;
    };
    let grounded = world
        .get::<PlatformerController>(walker)
        .is_some_and(PlatformerController::grounded);
    if !grounded
        || (!stride(world, walker, heading, pace) && !stride(world, walker, -heading, pace))
    {
        rest(world, walker);
    }
}

fn patrol(world: &mut World, enemy: Entity) {
    match kind(world, enemy) {
        Some(EnemyKind::Wisp) => wisp::patrol(world, enemy),
        Some(EnemyKind::Crawler) => crawler::patrol(world, enemy),
        Some(EnemyKind::Beetle) => beetle::patrol(world, enemy),
        Some(EnemyKind::Bat) => bat::patrol(world, enemy),
        Some(EnemyKind::Troll) => troll::patrol(world, enemy),
        Some(EnemyKind::Goblin) => goblin::patrol(world, enemy),
        None => rest(world, enemy),
    }
}

/// Its tell on spotting the player: it stops and faces where the alert
/// came from; a wisp flares, a troll roars, a goblin cackles. A fresh
/// alert, from its patrol or its way home, never from a search, shows its
/// "!" for [`ALERT_MARK_SECS`] and stings: the cue's interval makes the
/// spotter's and its neighbours' one sting.
fn enter_alert(world: &mut World, enemy: Entity) {
    rest(world, enemy);
    let point = world
        .get::<Brain>(enemy)
        .and_then(|brain| brain.alert.or(brain.last_seen));
    if let Some(point) = point {
        face(world, enemy, point[0]);
    }
    // No state before: its first visit, out of its first patrol.
    let fresh = world
        .get::<StateMachine<EnemyState>>(enemy)
        .is_some_and(|machine| {
            machine
                .previous()
                .is_none_or(|before| matches!(before, EnemyState::Patrol | EnemyState::Return))
        });
    if fresh {
        if let Some(brain) = world.get_mut::<Brain>(enemy) {
            brain.alerted = ALERT_MARK_SECS;
        }
        if let Some(at) = centre(world, enemy) {
            audio::play(world, Sound::Alerted, at);
        }
    }
    match kind(world, enemy) {
        Some(EnemyKind::Wisp) => wisp::flare(world, enemy),
        Some(EnemyKind::Troll) => troll::roar(world, enemy),
        Some(EnemyKind::Goblin) => goblin::cackle(world, enemy),
        _ => {}
    }
}

/// The alert lasts the kind's alert seconds, then the chase.
fn alert(world: &mut World, enemy: Entity) {
    let Some(kind) = kind(world, enemy) else {
        return;
    };
    rest(world, enemy);
    if elapsed(world, enemy) >= kind.info().alert {
        if let Some(brain) = world.get_mut::<Brain>(enemy) {
            // A neighbour's alert gives it the point to chase toward.
            if brain.last_seen.is_none() {
                brain.last_seen = brain.alert;
            }
            brain.alert = None;
        }
        go(world, enemy, EnemyState::Chase);
    }
}

fn chase(world: &mut World, enemy: Entity) {
    match kind(world, enemy) {
        Some(EnemyKind::Wisp) => wisp::chase(world, enemy),
        Some(EnemyKind::Crawler) => crawler::chase(world, enemy),
        Some(EnemyKind::Beetle) => beetle::chase(world, enemy),
        Some(EnemyKind::Bat) => bat::chase(world, enemy),
        Some(EnemyKind::Troll) => troll::chase(world, enemy),
        Some(EnemyKind::Goblin) => goblin::chase(world, enemy),
        None => go(world, enemy, EnemyState::Search),
    }
}

fn enter_windup(world: &mut World, enemy: Entity) {
    rest(world, enemy);
    match kind(world, enemy) {
        Some(EnemyKind::Crawler) => crawler::enter_windup(world, enemy),
        Some(EnemyKind::Beetle) => beetle::enter_windup(world, enemy),
        Some(EnemyKind::Bat) => bat::enter_windup(world, enemy),
        Some(EnemyKind::Goblin) => goblin::enter_windup(world, enemy),
        Some(EnemyKind::Troll) => troll::enter_windup(world, enemy),
        Some(EnemyKind::Wisp) | None => {}
    }
}

fn windup(world: &mut World, enemy: Entity) {
    match kind(world, enemy) {
        Some(EnemyKind::Crawler) => crawler::windup(world, enemy),
        Some(EnemyKind::Beetle) => beetle::windup(world, enemy),
        Some(EnemyKind::Bat) => bat::windup(world, enemy),
        Some(EnemyKind::Troll) => troll::windup(world, enemy),
        Some(EnemyKind::Goblin) => goblin::windup(world, enemy),
        Some(EnemyKind::Wisp) | None => go(world, enemy, EnemyState::Chase),
    }
}

fn enter_attack(world: &mut World, enemy: Entity) {
    match kind(world, enemy) {
        Some(EnemyKind::Crawler) => crawler::enter_attack(world, enemy),
        Some(EnemyKind::Beetle) => beetle::enter_attack(world, enemy),
        Some(EnemyKind::Bat) => bat::enter_attack(world, enemy),
        Some(EnemyKind::Troll) => troll::enter_attack(world, enemy),
        Some(EnemyKind::Goblin) => goblin::enter_attack(world, enemy),
        Some(EnemyKind::Wisp) | None => {}
    }
}

fn attack(world: &mut World, enemy: Entity) {
    match kind(world, enemy) {
        Some(EnemyKind::Crawler) => crawler::attack(world, enemy),
        Some(EnemyKind::Beetle) => beetle::attack(world, enemy),
        Some(EnemyKind::Bat) => bat::attack(world, enemy),
        Some(EnemyKind::Troll) => troll::attack(world, enemy),
        Some(EnemyKind::Goblin) => goblin::attack(world, enemy),
        Some(EnemyKind::Wisp) | None => go(world, enemy, EnemyState::Recover),
    }
}

/// After an attack it stands for its kind's recovery, the step's slack
/// below it, then chases on sight or searches; a bat climbs back to its
/// roost first.
fn recover(world: &mut World, enemy: Entity) {
    let Some(kind) = kind(world, enemy) else {
        return;
    };
    if kind == EnemyKind::Bat {
        bat::recover(world, enemy);
        return;
    }
    rest(world, enemy);
    if elapsed(world, enemy) >= kind.info().recover - 1e-4 {
        let next = if sees(world, enemy) {
            EnemyState::Chase
        } else {
            EnemyState::Search
        };
        go(world, enemy, next);
    }
}

/// Its search begins at the point it last saw the player.
fn enter_search(world: &mut World, enemy: Entity) {
    if let Some(brain) = world.get_mut::<Brain>(enemy) {
        brain.search = brain.last_seen;
    }
}

/// It goes to the search's point and looks there for [`SEARCH_SECS`],
/// then goes home.
fn search(world: &mut World, enemy: Entity) {
    if elapsed(world, enemy) >= SEARCH_SECS {
        go(world, enemy, EnemyState::Return);
        return;
    }
    let point = world
        .get::<Brain>(enemy)
        .and_then(|brain| brain.search)
        .map(Vec2::from);
    match kind(world, enemy) {
        Some(EnemyKind::Wisp) => {
            wisp::seek(world, enemy, point);
        }
        Some(EnemyKind::Crawler) => {
            crawler::seek(world, enemy, point);
        }
        Some(EnemyKind::Beetle) => {
            beetle::seek(world, enemy, point);
        }
        Some(EnemyKind::Bat) => {
            bat::seek(world, enemy, point);
        }
        Some(EnemyKind::Troll) => {
            troll::seek(world, enemy, point);
        }
        Some(EnemyKind::Goblin) => {
            goblin::seek(world, enemy, point);
        }
        None => rest(world, enemy),
    }
}

/// Back home, then on patrol.
fn home(world: &mut World, enemy: Entity) {
    let Some(home) = world.get::<Enemy>(enemy).map(Enemy::home) else {
        return;
    };
    let arrived = match kind(world, enemy) {
        Some(EnemyKind::Wisp) => wisp::seek(world, enemy, Some(home)),
        Some(EnemyKind::Crawler) => crawler::seek(world, enemy, Some(home)),
        Some(EnemyKind::Beetle) => beetle::seek(world, enemy, Some(home)),
        Some(EnemyKind::Bat) => bat::seek(world, enemy, Some(home)),
        Some(EnemyKind::Troll) => troll::seek(world, enemy, Some(home)),
        Some(EnemyKind::Goblin) => goblin::seek(world, enemy, Some(home)),
        None => true,
    };
    if arrived {
        go(world, enemy, EnemyState::Patrol);
    }
}

/// Whether `enemy` thinks nothing this step, and so is held: dead, more than
/// [`DORMANT_PAST`] from the player (or with no player), frozen or stunned.
fn asleep(world: &World, enemy: Entity, player: Option<Vec2>) -> bool {
    let dead = world.get::<Health>(enemy).is_none_or(Health::is_dead);
    let far = match (player, centre(world, enemy)) {
        (Some(player), Some(at)) => at.distance(player) > DORMANT_PAST,
        _ => true,
    };
    dead || far || has_status(world, enemy, FROZEN) || has_status(world, enemy, STUNNED)
}

/// The enemies, in entity order.
fn enemies(world: &World) -> Vec<Entity> {
    let mut enemies: Vec<Entity> = world
        .query::<(Entity, &Enemy)>()
        .map(|(entity, _)| entity)
        .collect();
    enemies.sort_unstable_by_key(|entity| entity.id());
    enemies
}

/// The most bodies [`clear`]'s ray sees past.
const SEEN_PAST: usize = 8;

/// Whether a body hides what is behind it from an enemy's eye (Q47): a
/// crate, a keg, iron, a scrap, a dummy or a frozen enemy; never a ball, a
/// rock, a bomb, the player or a live enemy.
fn hides(world: &World, body: Entity) -> bool {
    world.has::<Crate>(body)
        || world.has::<Keg>(body)
        || world.has::<IronBlock>(body)
        || world.has::<IronScrap>(body)
        || world.has::<Dummy>(body)
        || (world.has::<Enemy>(body) && has_status(world, body, FROZEN))
}

/// Whether nothing blocks `seer`'s line from `from` to `to`: no solid tile,
/// no body in a wall layer, and no body that [`hides`] what is behind it.
/// One ray looks past the other bodies, going on from inside the body it
/// met, as [`solid`]'s does; past [`SEEN_PAST`] of them, a full pit's
/// balls, the rest of the line counts as clear.
#[must_use]
pub fn clear(world: &World, seer: Entity, from: Vec2, to: Vec2) -> bool {
    let delta = to - from;
    let distance = delta.length();
    if distance < 1.0 {
        return true;
    }
    let direction = delta / distance;
    let tiles = SpatialFilter::new().with_bodies(false);
    let walls = SpatialFilter::new().with_mask(WALL_LAYERS);
    let open = [tiles, walls].iter().all(|filter| {
        cast_ray(world, from, direction, distance, filter)
            .is_none_or(|hit| hit.distance >= distance - 1.0)
    });
    if !open {
        return false;
    }
    let bodies = SpatialFilter::new().with_tiles(false).excluding(seer);
    let (mut from, mut left) = (from, distance - 1.0);
    for _ in 0..=SEEN_PAST {
        let Some(hit) = cast_ray(world, from, direction, left, &bodies) else {
            return true;
        };
        if hit.entity.is_some_and(|body| hides(world, body)) {
            return false;
        }
        // A hair inside its edge, where the ray passes it.
        let into = hit.distance + 0.01;
        from += direction * into;
        left -= into;
        if left <= 0.0 {
            return true;
        }
    }
    true
}

/// `enemy`'s eye, mirrored about its body's centre as it faces, as its
/// drawing is ([`clips`]).
#[must_use]
pub fn eye(world: &World, enemy: Entity) -> Option<Vec2> {
    let at = world.get::<Position>(enemy)?.0;
    let mine = world.get::<Enemy>(enemy)?;
    let info = mine.kind.info();
    let eye = if mine.heading < 0.0 {
        Vec2::new(2.0 * info.centre.x - info.eye.x, info.eye.y)
    } else {
        info.eye
    };
    Some(at + eye)
}

/// `enemy_sight`, in the intent phase: every [`SIGHT_EVERY`] seconds an
/// awake enemy within its reach of the player casts from its eye to the
/// player's centre, over the tiles, the slabs and the bodies that hide
/// ([`clear`]); seeing sets where it last
/// saw the player, and a sight that turns on sends [`Spotted`].
pub fn sight(world: &mut World) {
    let dt = world.resource::<Time>().delta();
    if dt <= 0.0 {
        return;
    }
    let player = player(world);
    let mut spotted = Vec::new();
    for enemy in enemies(world) {
        if asleep(world, enemy, player.map(|(_, at)| at)) {
            continue;
        }
        let Some((player, target)) = player else {
            continue;
        };
        let (Some(kind), Some(eye)) = (kind(world, enemy), eye(world, enemy)) else {
            continue;
        };
        let Some(next) = world.get::<Sight>(enemy).map(|sight| sight.next - dt) else {
            continue;
        };
        if next > 1e-5 {
            if let Some(sight) = world.get_mut::<Sight>(enemy) {
                sight.next = next;
            }
            continue;
        }
        let sees = eye.distance(target) <= kind.info().sight && clear(world, enemy, eye, target);
        let was = world.get::<Sight>(enemy).is_some_and(|sight| sight.sees);
        if let Some(sight) = world.get_mut::<Sight>(enemy) {
            sight.next = SIGHT_EVERY;
            sight.sees = sees;
        }
        if sees {
            if let Some(brain) = world.get_mut::<Brain>(enemy) {
                brain.last_seen = Some(target.to_array());
            }
            if !was {
                spotted.push(Spotted {
                    spotter: enemy,
                    at: eye,
                    player,
                });
            }
        }
    }
    if let Some(queue) = world.get_resource_mut::<EventQueue<Spotted>>() {
        for event in spotted {
            queue.send(event);
        }
    }
}

/// Whether `state` is the chase or the attack it leads to.
fn chasing(state: EnemyState) -> bool {
    matches!(
        state,
        EnemyState::Alert
            | EnemyState::Chase
            | EnemyState::WindUp
            | EnemyState::Attack
            | EnemyState::Recover
    )
}

/// `enemy_alerts`, in the intent phase after the sight: each spotting gives
/// every awake enemy within [`ALERT_REACH`] of the one that saw, itself
/// too, that is not already chasing, the point it saw from; it enters its
/// alert at its next visit.
pub fn alerts(world: &mut World, cursor: &mut EventCursor<Spotted>) {
    let Some(queue) = world.get_resource::<EventQueue<Spotted>>() else {
        return;
    };
    let events: Vec<Spotted> = cursor.read(queue).copied().collect();
    let player = player(world).map(|(_, at)| at);
    for spotted in events {
        let Some(from) = centre(world, spotted.spotter) else {
            continue;
        };
        let target = world
            .get::<Brain>(spotted.spotter)
            .and_then(|brain| brain.last_seen);
        for enemy in enemies(world) {
            if asleep(world, enemy, player)
                || centre(world, enemy).is_none_or(|at| at.distance(from) > ALERT_REACH)
            {
                continue;
            }
            let Some(state) = world
                .get::<StateMachine<EnemyState>>(enemy)
                .map(|machine| machine.pending().unwrap_or(machine.state()))
            else {
                continue;
            };
            if chasing(state) {
                continue;
            }
            if let Some(brain) = world.get_mut::<Brain>(enemy) {
                brain.alert = target.or(Some(spotted.at.to_array()));
            }
            go(world, enemy, EnemyState::Alert);
        }
    }
}

/// `enemy_marks`, in the intent phase before the machine: each fresh
/// alert's "!" runs down by the step.
pub fn marks(world: &mut World) {
    let dt = world.resource::<Time>().delta();
    if dt <= 0.0 {
        return;
    }
    for brain in world.query_mut::<&mut Brain>() {
        if brain.alerted > 0.0 {
            brain.alerted = (brain.alerted - dt).max(0.0);
        }
    }
}

/// `enemy_hold`, in the intent phase before the machine: each enemy that
/// thinks nothing this step is held (dead, dormant, frozen or stunned) and
/// its mover put at rest, the others let go. A stun replaces a wind-up's
/// tell with the recovery, so the attack never lands; a freeze holds it,
/// so the tell resumes at the thaw with the time it had left. A frozen
/// flyer falls on a borrowed controller, its own mover set aside until the
/// thaw; a frozen bat shatters once it lands. A frozen enemy's touch is set
/// aside until the thaw too, so a frozen crawler chills nothing and a
/// thrown one hurts by the throw.
pub fn hold(world: &mut World) {
    let player = player(world).map(|(_, at)| at);
    for enemy in enemies(world) {
        let held = asleep(world, enemy, player);
        let frozen = has_status(world, enemy, FROZEN);
        let stunned = has_status(world, enemy, STUNNED);
        swap_driver(world, enemy, frozen);
        swap_touch(world, enemy, frozen);
        if frozen && kind(world, enemy) == Some(EnemyKind::Bat) {
            bat::land(world, enemy);
        }
        let Some(machine) = machine_of(world, enemy) else {
            continue;
        };
        if stunned && machine.state() == EnemyState::WindUp {
            machine.go(EnemyState::Recover);
        }
        machine.set_held(held);
        if held {
            rest(world, enemy);
        }
    }
}

/// A flyer's driver for its state: frozen, a borrowed controller with no
/// input, so it falls, its own mover set aside; thawed, its own mover back
/// where it is.
fn swap_driver(world: &mut World, enemy: Entity, frozen: bool) {
    let flyer = kind(world, enemy).is_some_and(|kind| kind.info().flyer);
    if !flyer {
        return;
    }
    if frozen {
        if let Some(mover) = world.get::<TopDownMover>(enemy).cloned() {
            world.remove_component::<TopDownMover>(enemy);
            world.insert(enemy, Displaced(mover));
            let borrowed = PlatformerController::new(1.0, 1.0).with_filter(super::walk_filter());
            world.insert(enemy, borrowed);
        }
    } else if let Some(Displaced(mover)) = world.get::<Displaced>(enemy).cloned() {
        world.remove_component::<Displaced>(enemy);
        world.remove_component::<PlatformerController>(enemy);
        world.insert(enemy, mover);
    }
}

/// An enemy's touch for its state: frozen, set aside; thawed, back with
/// its targets' times.
fn swap_touch(world: &mut World, enemy: Entity, frozen: bool) {
    if frozen {
        if let Some(touch) = world.remove_component::<ContactDamage>(enemy) {
            world.insert(enemy, HeldTouch(touch));
        }
    } else if let Some(HeldTouch(touch)) = world.remove_component::<HeldTouch>(enemy) {
        world.insert(enemy, touch);
    }
}

/// `enemy_deaths`, in the react phase after the step's reactions: each
/// enemy a hit killed stops hurting at once, since its despawn comes at the
/// next step's end and its touch and hitbox would act until then; it ends
/// in one burst by its state, and its sprite goes with it.
pub fn deaths(world: &mut World, cursor: &mut EventCursor<Damaged>) {
    let Some(events) = world.get_resource::<EventQueue<Damaged>>() else {
        return;
    };
    let killed: Vec<Entity> = cursor
        .read(events)
        .filter(|hit| hit.killed && world.has::<Enemy>(hit.entity))
        .map(|hit| hit.entity)
        .collect();
    for enemy in killed {
        world.remove_component::<ContactDamage>(enemy);
        world.remove_component::<HeldTouch>(enemy);
        world.remove_component::<Hitbox>(enemy);
        end(world, enemy);
    }
}

/// An enemy's one burst: a shatter if frozen, gore if an iron blow killed
/// it this step, a burn-out if burning, a poof otherwise; its sprite
/// hidden.
fn end(world: &mut World, enemy: Entity) {
    let Some(at) = centre(world, enemy) else {
        return;
    };
    if let Some(visibility) = world.get_mut::<Visibility>(enemy) {
        visibility.visible = false;
    }
    if has_status(world, enemy, FROZEN) {
        spawn_key_burst(world, "ex10_ice_shatter", at);
        elements::ice_pulse(world, at, 64.0, true);
    } else if elements::systems::iron_killed(world, enemy) {
        spawn_key_burst(world, "ex10_gore_burst", at);
    } else if has_status(world, enemy, BURNING) {
        spawn_key_burst(world, "ex10_burn_out", at);
    } else {
        spawn_key_burst(world, "ex10_poof", at);
    }
}

/// The clip `enemy` shows now: its kind's clip for its state, facing its
/// heading.
fn clip(world: &World, enemy: Entity) -> Option<String> {
    let mine = world.get::<Enemy>(enemy)?;
    let state = world.get::<StateMachine<EnemyState>>(enemy)?.state();
    let clip = match mine.kind {
        EnemyKind::Wisp => wisp::clip(state),
        EnemyKind::Crawler => crawler::clip(world, enemy, state),
        EnemyKind::Beetle => beetle::clip(state),
        EnemyKind::Bat => bat::clip(world, enemy, state),
        EnemyKind::Troll => {
            let shown = world.get::<Shown>(enemy).map_or("", |shown| &shown.clip);
            troll::clip(world, enemy, state, shown)
        }
        EnemyKind::Goblin => goblin::clip(world, enemy, state),
    };
    let side = if mine.heading < 0.0 { "_left" } else { "" };
    Some(format!("ex10_{clip}{side}"))
}

/// The troll's walk frames its feet land on, where its steps sound.
const TROLL_STEPS: [usize; 2] = [0, 4];

/// `enemy_clips`, in the present phase after the kit's animation tick: each
/// enemy shows its state's clip, from its first frame when it changes, and
/// holds its frame while it thinks nothing; a wisp's flare lights a second
/// ember emitter, and a troll's feet sound as they land. A `_left` frame is
/// its right one mirrored, so a kind whose body is off its frame's centre
/// (the troll) draws a left frame shifted by twice the difference, its body
/// over its box; the physics sync writes the transform again each frame.
pub fn clips(world: &mut World) {
    let Some(registry) = world.remove_resource::<AnimationRegistry>() else {
        return;
    };
    for enemy in enemies(world) {
        let Some(wanted) = clip(world, enemy) else {
            continue;
        };
        let held = world
            .get::<StateMachine<EnemyState>>(enemy)
            .is_some_and(StateMachine::is_held);
        let current = world
            .get::<AnimationState>(enemy)
            .map(|state| state.animation_id.clone());
        if current.as_deref() != Some(wanted.as_str()) {
            let state = AnimationState::new(wanted.clone());
            let frame = state.current_sprite(&registry);
            world.insert(enemy, state);
            if let (Some(frame), Some(sprite)) = (frame, world.get_mut::<Sprite>(enemy)) {
                sprite.asset_id = frame;
            }
        }
        if let Some(state) = world.get_mut::<AnimationState>(enemy) {
            state.playing = !held;
        }
        steps(world, enemy, &wanted);
        if let Some(shown) = world.get_mut::<Shown>(enemy) {
            shown.clip = wanted;
        }
        mirror(world, enemy);
        wisp::flare_embers(world, enemy);
    }
    world.insert_resource(registry);
}

/// A troll's step, when its walk reaches a frame its foot lands on.
fn steps(world: &mut World, enemy: Entity, wanted: &str) {
    let frame = world
        .get::<AnimationState>(enemy)
        .map_or(0, |state| state.frame_index);
    let last = world
        .get::<Shown>(enemy)
        .map(|shown| (shown.frame, shown.clip == wanted));
    if let Some(shown) = world.get_mut::<Shown>(enemy) {
        shown.frame = frame;
    }
    let walking = wanted.starts_with("ex10_troll_walk");
    let landed = last.is_some_and(|(was, same)| !same || was != frame);
    if walking
        && landed
        && TROLL_STEPS.contains(&frame)
        && let Some(at) = centre(world, enemy)
    {
        audio::play(world, Sound::TrollStep, at);
    }
}

/// Shifts a left-facing enemy's drawing so its mirrored frame's body sits
/// over its box.
fn mirror(world: &mut World, enemy: Entity) {
    let Some(mine) = world.get::<Enemy>(enemy).copied() else {
        return;
    };
    let info = mine.kind.info();
    let shift = 2.0 * info.centre.x - info.frame;
    if mine.heading < 0.0
        && shift != 0.0
        && let Some(transform) = world.get_mut::<Transform>(enemy)
    {
        transform.position.x += shift;
    }
}
