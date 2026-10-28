//! The enemies (01 (c), (g)): a fire wisp, a frost crawler, a rust beetle,
//! a cave bat, a cave troll and a goblin sapper, from their prefabs, which
//! the map's `enemies` layer places and the summon slot summons. Each thinks on the kit's state machine (`D-158`): it patrols,
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
//! machine and sets a frozen one's touch aside, the difficulty's factor on
//! health and damage, a frozen flyer's
//! fall, a bat's roost, a troll's hold and throws, a goblin's throws, its
//! stab, its fidgets and its flight, a crawler's fear of the light, its
//! retreat and its snowball ([`snowball`]), the walkers' hops and leaps
//! ([`hop`]), the deaths' bursts,
//! their clips and the troll's bar. A run keeps its placed
//! enemies' save IDs, so a save names the dead as gone; the summon slot
//! places more, [`SUMMON_CAP`] at most, the oldest going.

pub mod bat;
pub mod beetle;
pub mod components;
pub mod crawler;
pub mod draw;
pub mod drops;
pub mod goblin;
pub mod hop;
pub mod snowball;
pub mod systems;
pub mod troll;
pub mod wisp;

use glam::Vec2;
use tungsten::core::{Entity, EventCursor, Position, Schedule, Shape, Stage, World, system};
use tungsten::kit::{
    ANIMATION_TICK, Health, KIT_DEBUG_DRAW, KNOCKBACK, Modifier, PICKUP_MAGNET, PICKUP_TICK,
    PLATFORMER_CONTROLLER, Phase, PickedUp, PlatformerController, Stats, TOP_DOWN_MOVER,
    ensure_phases,
};
use tungsten::physics::{SpatialFilter, overlap};
use tungsten::{Extracts, Plugin, SceneEntity};

use crate::features::combat;
use crate::features::level::LAVA_LAYER;
use crate::features::toys::components::PreviousVelocity;
use crate::game::GAMEPLAY;
use crate::shared::{SaveId, SaveIds, despawn_tree, spawn_key_burst};
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
/// The seconds an attack's recovery takes, but the troll's and the
/// goblin's own ([`KindInfo::recover`]).
pub const RECOVER_SECS: f32 = 0.5;
/// The seconds a fresh alert's "!" shows over an enemy, and how long a
/// search lasts before its "?" shows.
pub const ALERT_MARK_SECS: f32 = 0.8;
pub const SEARCH_MARK_AFTER: f32 = 0.4;
/// The most summoned enemies live at once (Q12, Q45), beside the dummies'
/// eight.
pub const SUMMON_CAP: usize = 10;
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
/// The heaviest body a walker shoves aside, by the kit's shove (N19): the
/// balls, a rock and a bomb, never a crate or a keg, which a fall from a
/// ledge would set off.
pub const WALK_SHOVE_MOST: f32 = 8.0;
/// The ledge or body a walker steps over on its way, half a tile: a ball,
/// a bowling ball, a rock or a bomb, never a crate or a keg.
pub const WALK_STEP: f32 = 32.0;

/// What an enemy's controller meets: everything but lava's surface, which
/// holds the player alone, so a walker or a frozen flyer falls in.
#[must_use]
pub fn walk_filter() -> SpatialFilter {
    SpatialFilter::new().with_mask(!LAVA_LAYER)
}

/// Gives `enemy`'s controller, when it has one, [`walk_filter`], and a
/// walker's its shove of up to [`WALK_SHOVE_MOST`] by its kind's mass and
/// its [`WALK_STEP`]: at a spawn, at a run's start for the map's, and after
/// a load, whose saved controller leaves its filter out. A frozen flyer's
/// borrowed controller takes the filter alone.
pub fn walk_on_rock(world: &mut World, enemy: Entity) {
    let walker = world
        .get::<Enemy>(enemy)
        .map(|mine| mine.kind.info())
        .filter(|info| !info.flyer);
    if let Some(controller) = world.get_mut::<PlatformerController>(enemy) {
        let mut set = controller.clone().with_filter(walk_filter());
        if let Some(info) = walker {
            set = set
                .with_shove(info.mass, WALK_SHOVE_MOST / info.mass)
                .with_step_height(WALK_STEP);
        }
        *controller = set;
    }
}

/// A kind's table entry: its prefab, its frame's side, its body's half
/// size, how far either side of its body's centre a lightning bolt meets
/// it (the opaque columns of its walk's or its flight's frames, mirrored
/// as it faces; Q49), its body's centre from its sprite's top-left (the
/// entity's point), its
/// eye from there facing right, its mass for impacts and the walkers'
/// shove, its sight's reach, its alert's seconds, the black hole's hold on
/// it, which a blast's push takes too, the force push's wave's weight on
/// it (`toys::wave`), its knockback's resist as its prefab sets it (the
/// kit's `Knockback` has no getter), its art, and its recovery's seconds
/// after an attack.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KindInfo {
    pub prefab: &'static str,
    pub frame: f32,
    pub half: Vec2,
    pub strike: f32,
    pub centre: Vec2,
    pub eye: Vec2,
    pub mass: f32,
    pub sight: f32,
    pub alert: f32,
    pub pull: f32,
    pub wave: f32,
    pub resist: f32,
    pub icon: &'static str,
    pub ice: &'static str,
    pub flyer: bool,
    pub recover: f32,
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
                strike: 17.0,
                centre: Vec2::new(32.0, 34.0),
                eye: Vec2::new(40.0, 32.0),
                mass: 3.0,
                sight: 480.0,
                alert: 0.5,
                pull: 0.6,
                wave: 3.5,
                resist: 0.0,
                icon: "ex10_icon_wisp",
                ice: "ex10_status_ice_wisp",
                recover: RECOVER_SECS,
                flyer: true,
            },
            EnemyKind::Crawler => KindInfo {
                prefab: "crawler",
                frame: 64.0,
                half: Vec2::new(24.0, 10.0),
                strike: 32.0,
                centre: Vec2::new(32.0, 51.0),
                eye: Vec2::new(56.0, 48.0),
                mass: 40.0,
                sight: 400.0,
                alert: 0.4,
                pull: 0.15,
                wave: 1.0,
                resist: 0.0,
                icon: "ex10_icon_crawler",
                ice: "ex10_status_ice_crawler",
                recover: RECOVER_SECS,
                flyer: false,
            },
            EnemyKind::Beetle => KindInfo {
                prefab: "beetle",
                frame: 64.0,
                half: Vec2::new(24.0, 13.0),
                strike: 32.0,
                centre: Vec2::new(32.0, 48.0),
                eye: Vec2::new(57.0, 45.0),
                mass: 120.0,
                sight: 520.0,
                alert: 0.5,
                pull: 0.05,
                wave: 0.53,
                resist: 0.5,
                icon: "ex10_icon_beetle",
                ice: "ex10_status_ice_beetle",
                recover: RECOVER_SECS,
                flyer: false,
            },
            EnemyKind::Bat => KindInfo {
                prefab: "bat",
                frame: 64.0,
                half: Vec2::new(10.0, 9.0),
                strike: 31.0,
                centre: Vec2::new(32.0, 28.0),
                eye: Vec2::new(32.0, 21.0),
                mass: 2.0,
                sight: 440.0,
                alert: 0.4,
                pull: 0.7,
                wave: 4.0,
                resist: 0.0,
                icon: "ex10_icon_bat",
                ice: "ex10_status_ice_bat",
                recover: RECOVER_SECS,
                flyer: true,
            },
            EnemyKind::Troll => KindInfo {
                prefab: "troll",
                frame: 128.0,
                half: Vec2::new(28.0, 55.0),
                strike: 41.0,
                centre: Vec2::new(74.0, 70.0),
                eye: Vec2::new(95.0, 21.0),
                mass: 600.0,
                sight: 640.0,
                alert: 0.8,
                pull: 0.01,
                wave: 0.14,
                resist: 1.0,
                icon: "ex10_icon_troll",
                ice: "ex10_status_ice_troll",
                recover: troll::RECOVER,
                flyer: false,
            },
            EnemyKind::Goblin => KindInfo {
                prefab: "goblin",
                frame: 64.0,
                half: Vec2::new(9.0, 20.0),
                strike: 22.0,
                centre: Vec2::new(34.0, 41.0),
                eye: Vec2::new(41.0, 24.0),
                mass: 50.0,
                sight: 520.0,
                alert: 0.4,
                pull: 0.12,
                wave: 0.86,
                resist: 0.0,
                icon: "ex10_icon_goblin",
                ice: "ex10_status_ice_goblin",
                recover: goblin::RECOVER,
                flyer: false,
            },
        }
    }
}

/// The enemies as a plugin: the `enemy` and `orb` components their prefabs
/// name; the throws in flight; in the intent phase the sight, the alerts,
/// the marks' and the enemies' cooldowns, the hold and then a burning
/// goblin's panic before the state machine, whose hooks run each kind's
/// rules; in the move phase the
/// trolls' holds after the movers and the dropped orbs' fall after the
/// kit's magnet; in the overlap phase the orbs collected and the
/// crawlers' snowballs ([`snowball::snowballs`]); in the react
/// phase the deaths and where the dead fell; in `PreUpdate` the orbs'
/// adoption; in the present phase the clips after the kit's animation tick,
/// the beetles' dust, the dropped orbs' blink and, under F1, their hit
/// tests before the kit's debug draw; and the trolls' bars and slam rings
/// and the enemies' marks drawn over the sprites.
pub struct EnemiesPlugin;

impl Plugin for EnemiesPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        // Before the manifest loads, which checks each prefab's components.
        world.register_component::<Enemy>("enemy");
        world.register_component::<drops::Orb>("orb");
        world.register_event::<Spotted>();
        world.insert_resource(Throws::default());
        world.resource_mut::<Extracts>().add_sprites(draw::bars);
        world.resource_mut::<Extracts>().add_sprites(draw::charges);
        world.resource_mut::<Extracts>().add_sprites(draw::rings);
        world.resource_mut::<Extracts>().add_sprites(draw::marks);
        world.insert_resource(beetle::Dust::default());
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
        let chill = system("chill_grades", combat::systems::chill_grades).before(ENEMY_STATES);
        let marks = system("enemy_marks", systems::marks).before(ENEMY_STATES);
        let cooldowns = system("enemy_cooldowns", systems::cooldowns).before(ENEMY_STATES);
        let panics = system("goblin_panic", goblin::panics)
            .after("enemy_hold")
            .before(ENEMY_STATES);
        for desc in [sight, alerts, hold, fire, chill, marks, cooldowns, panics] {
            schedule.add(Stage::FixedUpdate, Phase::Intent.join(desc));
        }
        let holds = system("troll_holds", troll::holds)
            .after(PLATFORMER_CONTROLLER)
            .after(TOP_DOWN_MOVER)
            .after(KNOCKBACK);
        schedule.add(Stage::FixedUpdate, Phase::Move.join(holds));
        let fall = system("orb_fall", drops::fall).after(PICKUP_MAGNET);
        schedule.add(Stage::FixedUpdate, Phase::Move.join(fall));
        let mut picked = EventCursor::<PickedUp>::new();
        let collect = system("orb_collect", move |world: &mut World| {
            drops::collect(world, &mut picked);
        })
        .after(PICKUP_TICK);
        schedule.add(Stage::FixedUpdate, Phase::Overlap.join(collect));
        let snowballs = system("snowballs", snowball::snowballs);
        schedule.add(Stage::FixedUpdate, Phase::Overlap.join(snowballs));
        // Before the shell's save captures them, so a save holds them whole.
        let adopt = system("orb_adopt", drops::adopt).before("shell_save");
        schedule.add(Stage::PreUpdate, adopt);
        let mut cursor = EventCursor::new();
        let deaths = system("enemy_deaths", move |world: &mut World| {
            systems::deaths(world, &mut cursor);
        })
        .after("combat_reactions");
        schedule.add(Stage::FixedUpdate, Phase::React.join(deaths));
        let mut cursor = EventCursor::new();
        let fallen = system("orb_fallen", move |world: &mut World| {
            drops::fallen(world, &mut cursor);
        })
        .after("enemy_deaths");
        schedule.add(Stage::FixedUpdate, Phase::React.join(fallen));
        let clips = system("enemy_clips", systems::clips).after(ANIMATION_TICK);
        schedule.add(Stage::PostUpdate, Phase::Present.join(clips));
        schedule.add(
            Stage::PostUpdate,
            Phase::Present.join(system("beetle_dust", beetle::dust)),
        );
        schedule.add(
            Stage::PostUpdate,
            Phase::Present.join(system("orb_blink", drops::blink)),
        );
        let boxes = system("enemy_debug_boxes", draw::debug_boxes).before(KIT_DEBUG_DRAW);
        schedule.add(Stage::PostUpdate, Phase::Present.join(boxes));
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
    walk_on_rock(world, enemy);
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
    spawn_key_burst(world, "ex10_poof", at);
    let summoned: Vec<(u64, Entity)> = live(world)
        .into_iter()
        .filter(|&(id, _)| !placed(world, id))
        .collect();
    for &(_, oldest) in summoned
        .iter()
        .take(summoned.len().saturating_sub(SUMMON_CAP))
    {
        if let Some(centre) = centre(world, oldest) {
            spawn_key_burst(world, "ex10_poof", centre);
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
        walk_on_rock(world, entity);
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

/// Whether `entity` is an enemy whose kind resists knockback whole (the
/// troll): 01 gives it no push, since the kit's `knock_back` would launch
/// its controller at zero and lock its input.
#[must_use]
pub fn stands_firm(world: &World, entity: Entity) -> bool {
    world
        .get::<Enemy>(entity)
        .is_some_and(|enemy| enemy.kind.info().resist >= 1.0)
}

/// An enemy's body centre.
#[must_use]
pub fn centre(world: &World, enemy: Entity) -> Option<Vec2> {
    let at = world.get::<Position>(enemy)?.0;
    let kind = world.get::<Enemy>(enemy)?.kind;
    Some(at + kind.info().centre)
}
