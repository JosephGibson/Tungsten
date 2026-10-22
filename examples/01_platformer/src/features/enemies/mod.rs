//! The enemies (01 (c)): a fire wisp, a frost crawler, a rust beetle, a
//! cave bat and a cave troll, from their prefabs, which the map's `enemies`
//! layer places and the summon slot summons. Each thinks on the kit's state machine (`D-158`): it patrols,
//! spots the player by line of sight, alerts its neighbours, chases,
//! attacks after a tell, searches where it last saw the player and returns
//! home. Their prefabs hold their kit items: a kinematic body on a mover
//! (a flyer's top-down mover, a walker's platformer controller), health
//! with its death policy, stats, a hurtbox, a touch that hurts the player,
//! resistances, an ailment threshold, knockback, the hit flash they show
//! unlit and their cues' sounds.
//!
//! 01 adds what the kit has no item for: the sight (two casts, the tiles
//! and the slabs' layer), the alert passed to the enemies near the one that
//! saw, the hold that stops a dead, dormant, frozen or stunned enemy's
//! machine, the difficulty's factor on health and damage, a frozen flyer's
//! fall, a bat's roost, a troll's hold and throws, the deaths' bursts, their
//! clips and the troll's bar. A run keeps its placed
//! enemies' save IDs, so a save names the dead as gone; the summon slot
//! places more, [`SUMMON_CAP`] at most, the oldest going.

pub mod bat;
pub mod beetle;
pub mod components;
pub mod crawler;
pub mod draw;
pub mod drops;
pub mod systems;
pub mod troll;
pub mod wisp;

use glam::Vec2;
use tungsten::core::{Entity, EventCursor, Position, Schedule, Shape, Stage, World, system};
use tungsten::kit::{
    ANIMATION_TICK, CueEvent, CuesOnEvent, Health, KNOCKBACK, Modifier, PICKUP_TICK,
    PLATFORMER_CONTROLLER, Phase, PickedUp, Stats, TOP_DOWN_MOVER, ensure_phases,
};
use tungsten::physics::{SpatialFilter, overlap};
use tungsten::{Extracts, Plugin, SceneEntity};

use crate::features::toys::components::PreviousVelocity;
use crate::game::GAMEPLAY;
use crate::shared::{SaveId, SaveIds, despawn_tree, spawn_burst};
use crate::shell::Difficulty;
use components::{
    Brain, Enemy, EnemyKind, EnemyState, PlacedEnemies, Shown, Sight, Spotted, Throws,
};

/// The state machine's system, in the intent phase.
pub const ENEMY_STATES: &str = "enemy_states";
/// The seconds between two sight checks.
pub const SIGHT_EVERY: f32 = 0.2;
/// How far a spotting's alert reaches the enemies near the one that saw.
pub const ALERT_REACH: f32 = 320.0;
/// Past this distance from the player an enemy thinks nothing (Q7).
pub const DORMANT_PAST: f32 = 1200.0;
/// The seconds an enemy searches where it last saw the player.
pub const SEARCH_SECS: f32 = 3.0;
/// The seconds an attack's recovery takes.
pub const RECOVER_SECS: f32 = 0.5;
/// The most summoned enemies live at once (Q12), beside the dummies' four.
pub const SUMMON_CAP: usize = 6;
/// The source of the difficulty's modifiers on an enemy's stats.
pub const DIFFICULTY: &str = "difficulty";
/// The physics layers of the level's static walls besides its tiles
/// (`tools/level.py`): stone slabs, the mine's iron and wooden decks.
pub const SLAB_LAYER: u32 = 1 << 1;
pub const IRON_LAYER: u32 = 1 << 2;
pub const WOOD_LAYER: u32 = 1 << 3;
/// Every wall layer, which a sight line and a roost meet besides the tiles:
/// iron and wood block sight and hold a bat as a slab does.
pub const WALL_LAYERS: u32 = SLAB_LAYER | IRON_LAYER | WOOD_LAYER;
/// How close a mover counts as at its point.
pub const ARRIVED: f32 = 10.0;

/// A kind's table entry: its prefab, its frame's side, its body's half
/// size and centre from its sprite's top-left (the entity's point), its
/// eye from there facing right, its mass for impacts, its sight's reach,
/// its alert's seconds, the black hole's hold on it, and its art.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KindInfo {
    pub prefab: &'static str,
    pub frame: f32,
    pub half: Vec2,
    pub centre: Vec2,
    pub eye: Vec2,
    pub mass: f32,
    pub sight: f32,
    pub alert: f32,
    pub pull: f32,
    pub icon: &'static str,
    pub ice: &'static str,
    pub flyer: bool,
}

impl EnemyKind {
    /// Its table entry.
    #[must_use]
    pub const fn info(self) -> KindInfo {
        match self {
            EnemyKind::Wisp => KindInfo {
                prefab: "wisp",
                frame: 64.0,
                half: Vec2::new(12.0, 12.0),
                centre: Vec2::new(32.0, 34.0),
                eye: Vec2::new(40.0, 32.0),
                mass: 3.0,
                sight: 480.0,
                alert: 0.5,
                pull: 0.6,
                icon: "ex10_icon_wisp",
                ice: "ex10_status_ice_wisp",
                flyer: true,
            },
            EnemyKind::Crawler => KindInfo {
                prefab: "crawler",
                frame: 64.0,
                half: Vec2::new(24.0, 10.0),
                centre: Vec2::new(32.0, 51.0),
                eye: Vec2::new(56.0, 48.0),
                mass: 40.0,
                sight: 400.0,
                alert: 0.4,
                pull: 0.15,
                icon: "ex10_icon_crawler",
                ice: "ex10_status_ice_crawler",
                flyer: false,
            },
            EnemyKind::Beetle => KindInfo {
                prefab: "beetle",
                frame: 64.0,
                half: Vec2::new(24.0, 13.0),
                centre: Vec2::new(32.0, 48.0),
                eye: Vec2::new(57.0, 45.0),
                mass: 120.0,
                sight: 520.0,
                alert: 0.5,
                pull: 0.05,
                icon: "ex10_icon_beetle",
                ice: "ex10_status_ice_beetle",
                flyer: false,
            },
            EnemyKind::Bat => KindInfo {
                prefab: "bat",
                frame: 64.0,
                half: Vec2::new(10.0, 9.0),
                centre: Vec2::new(32.0, 28.0),
                eye: Vec2::new(32.0, 21.0),
                mass: 2.0,
                sight: 440.0,
                alert: 0.4,
                pull: 0.7,
                icon: "ex10_icon_bat",
                ice: "ex10_status_ice_bat",
                flyer: true,
            },
            EnemyKind::Troll => KindInfo {
                prefab: "troll",
                frame: 128.0,
                half: Vec2::new(28.0, 55.0),
                centre: Vec2::new(74.0, 70.0),
                eye: Vec2::new(95.0, 21.0),
                mass: 600.0,
                sight: 640.0,
                alert: 0.8,
                pull: 0.01,
                icon: "ex10_icon_troll",
                ice: "ex10_status_ice_troll",
                flyer: false,
            },
        }
    }
}

/// The enemies as a plugin: the `enemy` and `orb` components their prefabs
/// name; the `spotted` cue the alert sting plays from; the throws in
/// flight; in the intent phase the sight, the alerts and the hold before
/// the state machine, whose hooks run each kind's rules; in the move phase
/// the trolls' holds after the movers; in the overlap phase the orbs
/// collected; in the react phase the deaths; in `PreUpdate` the orbs'
/// adoption; in the present phase the clips after the kit's animation
/// tick; and the trolls' bars drawn over the sprites.
pub struct EnemiesPlugin;

impl Plugin for EnemiesPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        // Before the manifest loads, which checks each prefab's components.
        world.register_component::<Enemy>("enemy");
        world.register_component::<drops::Orb>("orb");
        world.register_event::<Spotted>();
        world.insert_resource(Throws::default());
        world.resource_mut::<Extracts>().add_sprites(draw::bars);
        CuesOnEvent::<Spotted>::new("spotted", |spotted: &Spotted| {
            Some(CueEvent::new(spotted.player, spotted.at).with_other(spotted.spotter))
        })
        .build(schedule, world);
        ensure_phases(schedule);
        systems::machine().build(schedule, world);
        let sight = system("enemy_sight", systems::sight).before("enemy_alerts");
        let mut cursor = EventCursor::new();
        let alerts = system("enemy_alerts", move |world: &mut World| {
            systems::alerts(world, &mut cursor);
        })
        .before("enemy_hold");
        let hold = system("enemy_hold", systems::hold).before(ENEMY_STATES);
        let fire = system("wisp_fire", wisp::fire).after(ENEMY_STATES);
        let chill = system("troll_chill", troll::chill).before(ENEMY_STATES);
        for desc in [sight, alerts, hold, fire, chill] {
            schedule.add(Stage::FixedUpdate, Phase::Intent.join(desc));
        }
        let holds = system("troll_holds", troll::holds)
            .after(PLATFORMER_CONTROLLER)
            .after(TOP_DOWN_MOVER)
            .after(KNOCKBACK);
        schedule.add(Stage::FixedUpdate, Phase::Move.join(holds));
        let mut picked = EventCursor::<PickedUp>::new();
        let collect = system("orb_collect", move |world: &mut World| {
            drops::collect(world, &mut picked);
        })
        .after(PICKUP_TICK);
        schedule.add(Stage::FixedUpdate, Phase::Overlap.join(collect));
        // Before the shell's save captures them, so a save holds them whole.
        let adopt = system("orb_adopt", drops::adopt).before("shell_save");
        schedule.add(Stage::PreUpdate, adopt);
        let mut cursor = EventCursor::new();
        let deaths = system("enemy_deaths", move |world: &mut World| {
            systems::deaths(world, &mut cursor);
        })
        .after("combat_reactions");
        schedule.add(Stage::FixedUpdate, Phase::React.join(deaths));
        let clips = system("enemy_clips", systems::clips).after(ANIMATION_TICK);
        schedule.add(Stage::PostUpdate, Phase::Present.join(clips));
    }
}

/// The difficulty's factor on an enemy's `max_health` and `damage`.
#[must_use]
pub const fn factor(difficulty: Difficulty) -> f32 {
    match difficulty {
        Difficulty::Easy => 0.75,
        Difficulty::Normal => 1.0,
        Difficulty::Hard => 1.5,
    }
}

/// Gives `enemy` the difficulty's factor on its health and damage, from the
/// source [`DIFFICULTY`]: at its spawn, never at a load, whose stats carry
/// it already.
pub fn scale(world: &mut World, enemy: Entity, difficulty: Difficulty) {
    let factor = factor(difficulty);
    if let Some(stats) = world.get_mut::<Stats>(enemy) {
        stats.remove_modifiers(DIFFICULTY);
        for stat in [Stats::MAX_HEALTH, Stats::DAMAGE] {
            stats.add_modifier(stat, DIFFICULTY, Modifier::Mul(factor));
        }
    }
}

/// Spawns an enemy of `kind` with its sprite's top-left at `corner`, owned
/// by gameplay, with the save ID `id`, at once: a summon's or a load's. Its
/// home is where it stands, a bat's the roost above.
pub fn spawn(world: &mut World, kind: EnemyKind, corner: Vec2, id: SaveId) -> Option<Entity> {
    let enemy = world
        .prepare_prefab(kind.info().prefab, corner.to_array())
        .ok()?
        .with((
            id,
            Brain::default(),
            Sight::default(),
            tungsten::kit::StateMachine::new(EnemyState::Patrol),
            Shown::default(),
            PreviousVelocity::default(),
            SceneEntity { state_id: GAMEPLAY },
        ))
        .spawn(world);
    let home = home(world, kind, corner + kind.info().centre);
    if let Some(mine) = world.get_mut::<Enemy>(enemy) {
        mine.home = home.to_array();
    }
    Some(enemy)
}

/// The home of an enemy of `kind` placed with its body's centre at `at`:
/// that point, or for a bat the roost above it.
#[must_use]
pub fn home(world: &World, kind: EnemyKind, at: Vec2) -> Vec2 {
    if kind == EnemyKind::Bat {
        bat::roost(world, at)
    } else {
        at
    }
}

/// Whether an enemy of `kind` fits with its body's centre at `at`: no solid
/// tile in its box.
#[must_use]
pub fn fits(world: &World, kind: EnemyKind, at: Vec2) -> bool {
    let mut met = Vec::new();
    let shape = Shape::Aabb {
        half_extents: kind.info().half,
    };
    overlap(
        world,
        shape,
        at,
        &SpatialFilter::new().with_bodies(false),
        &mut met,
    );
    met.is_empty()
}

/// Summons an enemy of `kind` with its body's centre at `at`, at the run's
/// difficulty, with a poof; past [`SUMMON_CAP`] summoned live, the oldest
/// goes, with its own.
pub fn summon(world: &mut World, kind: EnemyKind, at: Vec2) -> Option<Entity> {
    let difficulty =
        crate::shell::run_meta(world).map_or(Difficulty::Normal, |meta| meta.difficulty);
    let id = world.resource_mut::<SaveIds>().take();
    let enemy = spawn(world, kind, at - kind.info().centre, id)?;
    scale(world, enemy, difficulty);
    spawn_burst(world, "ex10_poof", at);
    let summoned: Vec<(u64, Entity)> = live(world)
        .into_iter()
        .filter(|&(id, _)| !placed(world, id))
        .collect();
    for &(_, oldest) in summoned
        .iter()
        .take(summoned.len().saturating_sub(SUMMON_CAP))
    {
        if let Some(centre) = centre(world, oldest) {
            spawn_burst(world, "ex10_poof", centre);
        }
        despawn_tree(world, oldest);
    }
    Some(enemy)
}

/// For a run the map has just placed: keeps the save IDs of its enemies,
/// sets each one's home ([`home`]) and its state machine, and gives each the
/// difficulty's factor.
pub fn start(world: &mut World, difficulty: Difficulty) {
    let placed: Vec<(Entity, u64)> = world
        .query::<(Entity, &Enemy, &SaveId)>()
        .map(|(entity, _, id)| (entity, id.0))
        .collect();
    for &(entity, _) in &placed {
        let at = centre(world, entity).unwrap_or_default();
        let home = world
            .get::<Enemy>(entity)
            .map_or(at, |enemy| home(world, enemy.kind, at));
        if let Some(enemy) = world.get_mut::<Enemy>(entity) {
            enemy.home = home.to_array();
        }
        world.insert_bundle(
            entity,
            (
                Brain::default(),
                Sight::default(),
                tungsten::kit::StateMachine::new(EnemyState::Patrol),
                Shown::default(),
                PreviousVelocity::default(),
            ),
        );
        scale(world, entity, difficulty);
    }
    let mut placed: Vec<u64> = placed.into_iter().map(|(_, id)| id).collect();
    placed.sort_unstable();
    world.insert_resource(PlacedEnemies(placed));
    world.insert_resource(Throws::default());
}

/// The live enemies, alive and not dead, with their save IDs, in save-ID
/// order.
#[must_use]
pub fn live(world: &World) -> Vec<(u64, Entity)> {
    let mut live: Vec<(u64, Entity)> = world
        .query::<(Entity, &Enemy, &SaveId, &Health)>()
        .filter(|(.., health)| !health.is_dead())
        .map(|(entity, _, id, _)| (id.0, entity))
        .collect();
    live.sort_unstable_by_key(|(id, _)| *id);
    live
}

/// Whether the enemy of save ID `id` is one the map placed this run.
#[must_use]
pub fn placed(world: &World, id: u64) -> bool {
    world
        .get_resource::<PlacedEnemies>()
        .is_some_and(|placed| placed.0.binary_search(&id).is_ok())
}

/// The save IDs of the map's enemies no longer alive: what a save names as
/// gone, so a load leaves them out.
#[must_use]
pub fn gone(world: &World) -> Vec<u64> {
    let live = live(world);
    world
        .get_resource::<PlacedEnemies>()
        .map(|placed| {
            placed
                .0
                .iter()
                .copied()
                .filter(|id| !live.iter().any(|(live, _)| live == id))
                .collect()
        })
        .unwrap_or_default()
}

/// An enemy's body centre.
#[must_use]
pub fn centre(world: &World, enemy: Entity) -> Option<Vec2> {
    let at = world.get::<Position>(enemy)?.0;
    let kind = world.get::<Enemy>(enemy)?.kind;
    Some(at + kind.info().centre)
}
