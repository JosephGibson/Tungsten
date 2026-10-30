//! The full pit (redo 7.3): the ball pit under the rope bridge poured with
//! small balls, then the player on the bridge's east deck over it and six
//! enemies awake about them, at Normal, every one durable so nothing dies
//! and the work holds. The frame-time test, the `e01-pit` benchmark row and
//! the counter guard set it up alike, the script through `fill_pit` and
//! `wake_foes` ([`script`](super::script)).

use glam::Vec2;
use tungsten::core::{Entity, Position, PrevPosition, Velocity, World};
use tungsten::kit::{Health, StateMachine, Stats};

use super::Run;
use crate::features::enemies::components::{EnemyKind, EnemyState};
use crate::features::level::{PIT, PIT_DECK, PIT_SHELVES, TILE};
use crate::features::{enemies, player, toys};
use crate::shared::SaveIds;
use crate::shell::Difficulty;

/// The participants' max health: every attack lands, the troll's slam on
/// enemies too, but nothing dies.
pub const DURABLE: f32 = 1_000_000.0;

/// The six enemies [`wake_foes`] spawned, in its cast's order.
pub struct PitFoes(pub [Entity; 6]);

/// What the pit holds: its balls, and of the six enemies, those alive and
/// those awake. With no [`PitFoes`], none of either.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PitCounts {
    pub balls: usize,
    pub foes_alive: usize,
    pub foes_awake: usize,
}

/// Pours small balls over the pit's interior until the world holds `count`,
/// at most [`toys::BALL_CAP`]: a grid of 16 px slots from the floor up,
/// clear of the climbing shelves. Returns the world's ball count.
pub fn fill(world: &mut World, count: usize) -> usize {
    let target = count.min(toys::BALL_CAP);
    let across = (((PIT.right - PIT.left) * TILE - 16.0) / 16.0) as usize;
    let mut slot = 0;
    while toys::ball_count(world) < target {
        let at = Vec2::new(
            PIT.left * TILE + 8.0 + (slot % across) as f32 * 16.0,
            PIT.row * TILE - 8.0 - (slot / across) as f32 * 16.0,
        );
        slot += 1;
        let on_shelf = PIT_SHELVES.iter().any(|shelf| {
            at.x > shelf.left * TILE - 8.0
                && at.x < shelf.right * TILE + 8.0
                && at.y > shelf.row * TILE - 8.0
                && at.y < shelf.row * TILE + 32.0
        });
        if !on_shelf {
            toys::spawn_ball(world, at, true);
        }
    }
    toys::ball_count(world)
}

/// The deck's middle, where the player's body centre stands.
fn deck_middle(world: &World) -> Vec2 {
    Vec2::new(
        f32::midpoint(PIT_DECK.left, PIT_DECK.right) * TILE,
        PIT_DECK.row * TILE - player::body(world).half.y - 0.5,
    )
}

/// Puts the player at rest on the deck's middle and six enemies within
/// 600 px of them, the walkers on the deck and the east rim beyond the
/// chute, never over the gap or a chute. Heals the player and makes all
/// seven durable: their max health becomes [`DURABLE`] at the next step,
/// their health kept full. Keeps the six in [`PitFoes`].
pub fn wake_foes(world: &mut World) -> Result<[Entity; 6], String> {
    let player = world.get_resource::<Run>().ok_or("no run")?.player;
    let middle = deck_middle(world);
    let at = middle - player::body(world).centre;
    world.get_mut::<Position>(player).ok_or("no player")?.0 = at;
    if let Some(prev) = world.get_mut::<PrevPosition>(player) {
        prev.0 = at;
    }
    if let Some(velocity) = world.get_mut::<Velocity>(player) {
        velocity.0 = Vec2::ZERO;
    }

    let surface = PIT_DECK.row * TILE;
    let walk = |world: &World, kind: EnemyKind, dx: f32| {
        Vec2::new(
            middle.x + dx,
            surface - enemies::body(world, kind).half.y - 0.5,
        )
    };
    let cast = [
        (EnemyKind::Wisp, middle + Vec2::new(-220.0, -200.0)),
        (EnemyKind::Crawler, walk(world, EnemyKind::Crawler, -150.0)),
        (EnemyKind::Beetle, walk(world, EnemyKind::Beetle, 544.0)),
        (EnemyKind::Bat, middle + Vec2::new(120.0, -260.0)),
        (EnemyKind::Troll, walk(world, EnemyKind::Troll, 700.0)),
        (EnemyKind::Wisp, middle + Vec2::new(260.0, -180.0)),
    ];
    let mut six = [player; 6];
    for (slot, (kind, centre)) in six.iter_mut().zip(cast) {
        let id = world.resource_mut::<SaveIds>().take();
        let corner = centre - enemies::body(world, kind).centre;
        let enemy = enemies::spawn(world, kind, corner, id).ok_or("an enemy's prefab")?;
        enemies::scale(world, enemy, Difficulty::Normal);
        *slot = enemy;
    }
    for entity in six.into_iter().chain([player]) {
        if let Some(health) = world.get_mut::<Health>(entity) {
            let max = health.max();
            health.heal(max);
        }
        if let Some(stats) = world.get_mut::<Stats>(entity) {
            stats.set_base(Stats::MAX_HEALTH, DURABLE);
        }
    }
    world.insert_resource(PitFoes(six));
    Ok(six)
}

/// The pit's counts as the frame-time test reads them: a foe is alive while
/// its health is not dead and awake while its state machine is not held.
#[must_use]
pub fn counts(world: &World) -> PitCounts {
    let foes = world
        .get_resource::<PitFoes>()
        .map_or(&[][..], |foes| &foes.0);
    PitCounts {
        balls: toys::ball_count(world),
        foes_alive: foes
            .iter()
            .filter(|&&foe| world.get::<Health>(foe).is_some_and(|h| !h.is_dead()))
            .count(),
        foes_awake: foes
            .iter()
            .filter(|&&foe| {
                world
                    .get::<StateMachine<EnemyState>>(foe)
                    .is_some_and(|machine| !machine.is_held())
            })
            .count(),
    }
}
