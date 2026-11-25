//! The full pit (redo 7.3): the ball pit under the rope bridge poured with
//! small balls, then the player on the bridge's east deck over it and six
//! enemies awake about them, at Normal, every one durable so nothing dies
//! and the work holds. The frame-time test, the `e01-pit` benchmark row and
//! the counter guard set it up alike, the script through `fill_pit` and
//! `wake_foes` ([`script`](super::script)). The `e01-stress` row then lands
//! spells on it through `cast_at` ([`cast_at`]). Where the pit is and where
//! its cast stands are the map's places (`tools/level.py`'s `places`
//! layer), read by name. The `e01-shock` row sprays cold on it through
//! `cast_at` and reads its cold and its chains through [`shock`].

use glam::Vec2;
use tungsten::core::{Entity, Position, PrevPosition, Velocity, World};
use tungsten::kit::{Health, StateMachine, Stats};

use super::Run;
use crate::features::combat::{BURNING, FROZEN};
use crate::features::elements::cascade::Cascades;
use crate::features::elements::components::Flame;
use crate::features::enemies::components::{EnemyKind, EnemyState, Variant};
use crate::features::toys::components::{Element, SmallBall};
use crate::features::{enemies, level, lightning, player, toys};
use crate::shell::Difficulty;
use tungsten::kit::Tags;

/// The participants' max health: every attack lands, the troll's slam on
/// enemies too, but nothing dies.
pub const DURABLE: f32 = 1_000_000.0;

/// The map's places the full pit reads: the pit's interior and each of
/// its climbing shelves, rectangles; the player's feet on the deck over
/// it; the walkers' feet on that deck's surface; and the flyers' centres,
/// the two wisps in the order the cast takes them.
const PIT_INTERIOR: &str = "pit";
const PIT_SHELF: &str = "pit_shelf";
const PIT_PLAYER: &str = "pit_player";
const PIT_CRAWLER: &str = "pit_crawler";
const PIT_BEETLE: &str = "pit_beetle";
const PIT_TROLL: &str = "pit_troll";
const PIT_BAT: &str = "pit_bat";
const PIT_WISP: &str = "pit_wisp";

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

/// Pours small balls over the pit's interior, the map's `pit` place, until
/// the world holds `count`, at most [`toys::BALL_CAP`]: a grid of 16 px
/// slots from the floor up, clear of the climbing shelves, its `pit_shelf`
/// places. Returns the world's ball count, which a map without the place
/// leaves as it was.
pub fn fill(world: &mut World, count: usize) -> usize {
    let target = count.min(toys::BALL_CAP);
    let Some(&(min, max)) = level::marker_boxes(world, PIT_INTERIOR).first() else {
        return toys::ball_count(world);
    };
    let shelves = level::marker_boxes(world, PIT_SHELF);
    let across = (((max.x - min.x) - 16.0) / 16.0) as usize;
    if across == 0 {
        return toys::ball_count(world);
    }
    let mut slot = 0;
    while toys::ball_count(world) < target {
        let at = Vec2::new(
            min.x + 8.0 + (slot % across) as f32 * 16.0,
            max.y - 8.0 - (slot / across) as f32 * 16.0,
        );
        slot += 1;
        let on_shelf = shelves.iter().any(|(low, high)| {
            at.x > low.x - 8.0 && at.x < high.x + 8.0 && at.y > low.y - 8.0 && at.y < low.y + 32.0
        });
        if !on_shelf {
            toys::spawn_ball(world, at, true);
        }
    }
    toys::ball_count(world)
}

/// The map's place `name`, a point.
fn place(world: &World, name: &str) -> Result<Vec2, String> {
    level::marker(world, name).ok_or_else(|| format!("the map has no `{name}` place"))
}

/// Puts the player at rest on the deck's middle and six enemies within
/// 600 px of them, the walkers on the deck and the east rim beyond the
/// chute, never over the gap or a chute, each at its place on the map.
/// Heals the player and makes all
/// seven durable: their max health becomes [`DURABLE`] at the next step,
/// their health kept full. Keeps the six in [`PitFoes`].
pub fn wake_foes(world: &mut World) -> Result<[Entity; 6], String> {
    let player = world.get_resource::<Run>().ok_or("no run")?.player;
    // A stander's body centre over its feet's place.
    let feet = place(world, PIT_PLAYER)?;
    let middle = Vec2::new(feet.x, feet.y - player::body(world).half.y - 0.5);
    let at = middle - player::body(world).centre;
    world.get_mut::<Position>(player).ok_or("no player")?.0 = at;
    if let Some(prev) = world.get_mut::<PrevPosition>(player) {
        prev.0 = at;
    }
    if let Some(velocity) = world.get_mut::<Velocity>(player) {
        velocity.0 = Vec2::ZERO;
    }

    let walk = |world: &World, kind: EnemyKind, name: &str| {
        place(world, name)
            .map(|feet| Vec2::new(feet.x, feet.y - enemies::body(world, kind).half.y - 0.5))
    };
    let [west, east] = level::markers(world, PIT_WISP)[..] else {
        return Err(format!("the map has not two `{PIT_WISP}` places"));
    };
    let cast = [
        (EnemyKind::Wisp, west),
        (
            EnemyKind::Crawler,
            walk(world, EnemyKind::Crawler, PIT_CRAWLER)?,
        ),
        (
            EnemyKind::Beetle,
            walk(world, EnemyKind::Beetle, PIT_BEETLE)?,
        ),
        (EnemyKind::Bat, place(world, PIT_BAT)?),
        (EnemyKind::Troll, walk(world, EnemyKind::Troll, PIT_TROLL)?),
        (EnemyKind::Wisp, east),
    ];
    let mut six = [player; 6];
    for (slot, (kind, centre)) in six.iter_mut().zip(cast) {
        let corner = centre - enemies::body(world, kind).centre;
        let enemy = enemies::spawn_record(world, kind, Variant::Plain, corner)
            .ok_or("an enemy's prefab")?;
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

/// Lands `spell`'s toy at `at` as a cast of the wizard's would, free of its
/// ability's mana, cooldown and charges, so a script casts as often as its
/// row asks: `fire_missile` blasts there, `ice_spray` holds the wizard's
/// cold spray toward it for the step, `lightning` strikes down onto it,
/// `force_push` sends its wave from the wizard toward it, and a
/// `black_hole` or an `iron_block` is placed there, the block refused where
/// it would not fit.
pub fn cast_at(world: &mut World, spell: &str, at: Vec2) -> Result<(), String> {
    let player = world.get_resource::<Run>().ok_or("no run")?.player;
    let centre = player::centre(world, player).ok_or("no player")?;
    match spell {
        "fire_missile" => toys::systems::explode(world, at),
        "ice_spray" => toys::spray(world, player, at - centre, Element::Cold),
        "lightning" => {
            lightning::strike(world, player, at);
        }
        "force_push" => toys::wave::cast(world, centre, at - centre),
        "black_hole" => {
            toys::place_black_hole(world, at);
        }
        "iron_block" => {
            if !toys::iron_fits(world, at) {
                return Err(format!("no room for an iron block at {at}"));
            }
            toys::place_iron(world, at);
        }
        other => return Err(format!("no spell `{other}` to cast at a point")),
    }
    Ok(())
}

/// The pit's cold among its fire: the small balls frozen now, each
/// carrying `status.frozen`, and the thermal shocks and steam bursts the
/// run's chains have set off so far ([`Cascades`]' counts).
#[must_use]
pub fn shock(world: &World) -> (usize, u64, u64) {
    let frozen = world
        .query::<(&SmallBall, Option<&Tags>)>()
        .filter(|(_, tags)| tags.is_some_and(|tags| tags.has(FROZEN)))
        .count();
    let (shocks, steams) = world
        .get_resource::<Cascades>()
        .map_or((0, 0), |cascades| (cascades.shocks, cascades.steams));
    (frozen, shocks, steams)
}

/// The small balls burning now, and those that have burned out: each lit
/// ball's flame, burning while it carries `status.burning`.
#[must_use]
pub fn fire(world: &World) -> (usize, usize) {
    let lit = || world.query::<(&Flame, Option<&Tags>)>();
    let burning = lit()
        .filter(|(_, tags)| tags.is_some_and(|tags| tags.has(BURNING)))
        .count();
    (burning, lit().count() - burning)
}
