//! The enemies (01 (c), (g)): a fire wisp, a frost crawler, a rust beetle,
//! a cave bat, a cave troll and a goblin sapper, from their prefabs, which
//! the map's `enemies` layer places and the summon slot summons. Each thinks on the kit's state machine (`D-158`): it patrols,
//! spots the player by line of sight, alerts its neighbours, chases,
//! attacks after a tell, searches where it last saw the player and returns
//! home. Their prefabs hold their kit items: a kinematic body on a mover
//! (a flyer's top-down mover, a walker's platformer controller), health
//! with its death policy, stats, a hurtbox, a touch that hurts the player,
//! resistances, a status threshold, knockback, the hit flash they show
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
//! ([`hop`]), the deaths' bursts, and the tags their prefabs' clip rules
//! read, the troll's steps and its shifted drawing; the kit's animator
//! shows their clips and draws the troll's bar. A run keeps its placed
//! enemies' save IDs, so a save names the dead as gone; the summon slot
//! places more, [`SUMMON_CAP`] at most, the oldest going.

pub mod armour;
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
use serde::Deserialize;
use tungsten::core::save::{SaveOptions, Value};
use tungsten::core::{
    AnimationRegistry, AssetRegistry, DataRegistry, Entity, EventCursor, Position, PrefabRegistry,
    Schedule, Shape, Stage, World, system,
};
use tungsten::kit::{
    CHARACTER_ANIMATOR, EFFECT_LOOK_LATE, FOLLOW, Facing, Health, KIT_DEBUG_DRAW, KNOCKBACK, Line,
    Modifier, PLATFORMER_CONTROLLER, PROJECTILE_TICK, Phase, Stats, Steering, TOP_DOWN_MOVER,
    TagFilter, WorldSource, WorldSources, ensure_phases,
};
use tungsten::physics::{SpatialFilter, overlap};
use tungsten::{Extracts, Plugin, SceneEntity};

use crate::features::combat;
use crate::features::level::{self, LAVA_LAYER};
use crate::game::GAMEPLAY;
use crate::shared::{self, BODIES, Body, Saved, despawn_tree, spawn_key_burst};
use crate::shell::Difficulty;
use components::{
    Brain, Enemy, EnemyKind, EnemyState, PlacedEnemies, Shown, Sight, Spotted, Throws, Variant,
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
/// How long a search lasts before its "?" shows; a fresh alert's "!" is
/// its `alert_mark` effect's seconds.
pub const SEARCH_MARK_AFTER: f32 = 0.4;
/// The "!" and "?" marks' sprites, which the foes' prefabs' looks show.
const MARK_SPRITES: [&str; 2] = ["ex10_mark_alert", "ex10_mark_search"];
/// The most summoned enemies live at once (Q12, Q45), beside the dummies'
/// eight.
pub const SUMMON_CAP: usize = 10;
/// The world source of the difficulty's lines on the enemies' stats, and
/// the key they sit under.
pub const DIFFICULTY: &str = "difficulty";
/// The tag every enemy carries, which [`DIFFICULTY`]'s filter names.
const TEAM_ENEMY: &str = "team.enemy";
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

/// The data kind of the foes' kinds ([`KindInfo`]), each by its kind's
/// name (`data/enemy_kinds/<kind>.json`).
pub const ENEMY_KINDS: &str = "enemy_kinds";

/// A kind's entry in the [`ENEMY_KINDS`] data: how far either side of its
/// body's centre a lightning bolt meets it (the opaque columns of its
/// walk's or its flight's frames, mirrored as it faces; Q49), its mass for
/// impacts and the walkers' shove, its sight's reach, its alert's seconds,
/// the black hole's hold on it, which a blast's push takes too, the force
/// push's wave's weight on it (`toys::wave`), its ice cube's art, whether
/// it flies, and its recovery's seconds after an attack; its knockback's
/// resist is its prefab's alone (`Knockback::resist`), and its prefab and
/// summon icon are its name's ([`prefab`], [`icon`]). Checked as it loads:
/// every number finite, its mass above 0 and the rest at least 0, a refused
/// entry failing the load at its field.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(try_from = "KindForm")]
pub struct KindInfo {
    pub strike: f32,
    pub mass: f32,
    pub sight: f32,
    pub alert: f32,
    pub pull: f32,
    pub wave: f32,
    pub ice: String,
    pub flyer: bool,
    pub recover: f32,
}

/// [`KindInfo`] as its file writes it, before its numbers are checked.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct KindForm {
    strike: f32,
    mass: f32,
    sight: f32,
    alert: f32,
    pull: f32,
    wave: f32,
    ice: String,
    flyer: bool,
    recover: f32,
}

impl TryFrom<KindForm> for KindInfo {
    type Error = String;

    fn try_from(form: KindForm) -> Result<Self, Self::Error> {
        if !(form.mass.is_finite() && form.mass > 0.0) {
            return Err(format!("mass: must be finite and > 0 (got {})", form.mass));
        }
        for (field, value) in [
            ("strike", form.strike),
            ("sight", form.sight),
            ("alert", form.alert),
            ("pull", form.pull),
            ("wave", form.wave),
            ("recover", form.recover),
        ] {
            if !(value.is_finite() && value >= 0.0) {
                return Err(format!("{field}: must be finite and >= 0 (got {value})"));
            }
        }
        Ok(Self {
            strike: form.strike,
            mass: form.mass,
            sight: form.sight,
            alert: form.alert,
            pull: form.pull,
            wave: form.wave,
            ice: form.ice,
            flyer: form.flyer,
            recover: form.recover,
        })
    }
}

/// `kind`'s entry in the [`ENEMY_KINDS`] data, as it is now.
///
/// # Panics
///
/// When the data has no entry of that kind: every kind's loads with the
/// manifest.
#[must_use]
pub fn info(world: &World, kind: EnemyKind) -> &KindInfo {
    world
        .get_resource::<DataRegistry>()
        .and_then(|data| data.get::<KindInfo>(kind.name()))
        .unwrap_or_else(|| panic!("no {ENEMY_KINDS} entry for {}", kind.name()))
}

/// The enemies as a plugin: the `enemy` and `orb` components their prefabs
/// name; the throws in flight; in the intent phase the sight, the alerts,
/// the hold and then a burning goblin's panic before the state machine,
/// whose hooks run each kind's rules, the "!" and the rests on the kit's
/// effects and cooldowns; in the move phase the
/// trolls' holds after the movers and the dropped orbs' fall after the
/// kit's magnet; in the overlap phase the orbs collected and the
/// crawlers' snowballs' bursts after the kit's projectile step
/// ([`snowball::ends`]); in the react
/// phase the deaths, the tags the foes' clip rules read
/// ([`systems::foe_tags`]) and where the dead fell; in `PreUpdate` the
/// orbs' adoption; in the present phase the trolls' steps and shifted
/// drawing after the kit's animator and before its late look pass, the
/// dropped orbs' blink and, under F1, their hit tests before the kit's
/// debug draw; and the trolls' slam rings drawn over the sprites. The
/// marks, the beetle's streaks and dust and the wisp's flare are the
/// prefabs' looks.
pub struct EnemiesPlugin;

impl Plugin for EnemiesPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        // Before the manifest loads, which checks each prefab's components.
        world.register_component::<Enemy>("enemy");
        // A foe's mind and its looks' bookkeeping, fresh in its prefab.
        world.register_component::<Brain>("brain");
        world.register_component::<Sight>("sight");
        world.register_component::<tungsten::kit::StateMachine<EnemyState>>("enemy_state");
        world.register_component::<Shown>("shown");
        world.register_component::<drops::Orb>("orb");
        world.register_saved::<Enemy>("enemy");
        world.register_saved::<Brain>("brain");
        world.register_saved::<Sight>("sight");
        world.register_saved::<tungsten::kit::StateMachine<EnemyState>>("enemy_state");
        world.register_saved::<components::Displaced>("displaced");
        world.register_saved::<drops::Orb>("orb");
        world.register_saved_with(
            "troll_hold",
            SaveOptions::<components::Holding>::form::<components::HoldingForm>()
                .refs(components::holding_refs()),
        );
        world.register_component::<components::Snowball>("snowball");
        world.register_saved_resource_with(
            "throws",
            SaveOptions::<Throws>::form::<components::ThrowsForm>().refs(components::throws_refs()),
        );
        world.register_data_kind::<Body>(BODIES);
        world.register_data_kind::<KindInfo>(ENEMY_KINDS);
        // The marks' plain sprites, which the foes' looks name: interned
        // before the manifest loads and checks each look's sprite. A load
        // interns the manifest's every sprite first; the headless harness
        // only those its animations, particles and prefabs' own sprites
        // name (known issues).
        world
            .resource_mut::<AssetRegistry>()
            .intern_sprites(MARK_SPRITES);
        world.register_event::<Spotted>();
        world.insert_resource(Throws::default());
        world.resource_mut::<Extracts>().add_sprites(draw::rings);
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
        let panics = system("goblin_panic", goblin::panics)
            .after("enemy_hold")
            .before(ENEMY_STATES);
        for desc in [sight, alerts, hold, fire, chill, panics] {
            schedule.add(Stage::FixedUpdate, Phase::Intent.join(desc));
        }
        let holds = system("troll_holds", troll::holds)
            .after(PLATFORMER_CONTROLLER)
            .after(TOP_DOWN_MOVER)
            .after(KNOCKBACK)
            .after(FOLLOW);
        schedule.add(Stage::FixedUpdate, Phase::Move.join(holds));
        let snowball_ends = system("snowball_ends", {
            let mut cursor = EventCursor::new();
            move |world: &mut World| snowball::ends(world, &mut cursor)
        })
        .after(PROJECTILE_TICK);
        schedule.add(Stage::FixedUpdate, Phase::Overlap.join(snowball_ends));
        // Before the shell's save captures them, so a save holds them whole.
        let adopt = system("orb_adopt", drops::adopt).before("shell_save");
        schedule.add(Stage::PreUpdate, adopt);
        let mut cursor = EventCursor::new();
        let deaths = system("enemy_deaths", move |world: &mut World| {
            systems::deaths(world, &mut cursor);
        })
        .after("combat_reactions");
        schedule.add(Stage::FixedUpdate, Phase::React.join(deaths));
        let mut foes = Vec::new();
        let tags = system("foe_tags", move |world: &mut World| {
            systems::foe_tags(world, &mut foes);
        })
        .after("enemy_deaths");
        schedule.add(Stage::FixedUpdate, Phase::React.join(tags));
        let clips = system("enemy_clips", systems::clips)
            .after(CHARACTER_ANIMATOR)
            .before(EFFECT_LOOK_LATE);
        schedule.add(Stage::PostUpdate, Phase::Present.join(clips));
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

/// Gives `enemy` the difficulty's factor on its health and damage: sets the
/// world source [`DIFFICULTY`] (`team.enemy`, `mul` [`factor`] on
/// `max_health` and `damage`), which an equal one leaves alone, and brings
/// `enemy` to it at once; the rest of the living take it at the next step.
/// A saved enemy's lines from it come back with its stats and stay where
/// they are.
pub fn scale(world: &mut World, enemy: Entity, difficulty: Difficulty) {
    let factor = factor(difficulty);
    let lines = [
        (Stats::DAMAGE, Modifier::Mul(factor)),
        (Stats::MAX_HEALTH, Modifier::Mul(factor)),
    ];
    let set = world
        .get_resource::<WorldSources>()
        .and_then(|sources| sources.get(DIFFICULTY))
        .is_some_and(|source| {
            source
                .lines()
                .map(|(stat, line)| (stat, line.modifier()))
                .eq(lines)
        });
    if !set {
        let source = lines.into_iter().fold(
            WorldSource::new(TagFilter::new().all([TEAM_ENEMY])),
            |source, (stat, modifier)| source.with_line(stat, Line::new(modifier)),
        );
        WorldSources::set(world, DIFFICULTY, source);
    }
    WorldSources::apply_to(world, enemy);
}

/// The prefab of `kind`'s `variant`: the kind's own, or the armoured
/// troll's.
#[must_use]
pub const fn prefab(kind: EnemyKind, variant: Variant) -> &'static str {
    match (kind, variant) {
        (EnemyKind::Troll, Variant::Armoured) => "troll_armoured",
        _ => kind.name(),
    }
}

/// The summon icon of `kind`'s `variant`: the kind's own, or the armoured
/// troll's.
#[must_use]
pub const fn icon(kind: EnemyKind, variant: Variant) -> &'static str {
    match (kind, variant) {
        (EnemyKind::Troll, Variant::Armoured) => "ex10_icon_troll_armoured",
        (EnemyKind::Wisp, _) => "ex10_icon_wisp",
        (EnemyKind::Crawler, _) => "ex10_icon_crawler",
        (EnemyKind::Beetle, _) => "ex10_icon_beetle",
        (EnemyKind::Bat, _) => "ex10_icon_bat",
        (EnemyKind::Troll, _) => "ex10_icon_troll",
        (EnemyKind::Goblin, _) => "ex10_icon_goblin",
    }
}

/// Spawns a plain enemy of `kind` with its sprite's top-left at `corner`
/// ([`spawn_as`]).
pub fn spawn(world: &mut World, kind: EnemyKind, corner: Vec2) -> Option<Entity> {
    spawn_as(world, kind, Variant::Plain, corner)
}

/// Spawns an enemy of `kind`'s `variant` with its sprite's top-left at
/// `corner`, one of the run's records ([`spawn_record`]), which takes the
/// run's next save ID at its spawn: a test's.
pub fn spawn_as(
    world: &mut World,
    kind: EnemyKind,
    variant: Variant,
    corner: Vec2,
) -> Option<Entity> {
    spawn_record(world, kind, variant, corner)
}

/// Spawns an enemy of `kind`'s `variant` with its sprite's top-left at
/// `corner`, owned by gameplay, as one of the run's records
/// (`PreparedPrefab::saved`, the map's area), at once: a summon's or a
/// test's. Its body is its kind's data's ([`fit`]), and its home where it
/// stands, a bat's the roost above.
pub fn spawn_record(
    world: &mut World,
    kind: EnemyKind,
    variant: Variant,
    corner: Vec2,
) -> Option<Entity> {
    let enemy = world
        .prepare_prefab(prefab(kind, variant), corner.to_array())
        .ok()?
        .with((SceneEntity { state_id: GAMEPLAY },))
        .saved(world, Some(level::MAP))
        .spawn(world);
    fit(world, enemy);
    let home = home(world, kind, corner + body(world, kind).centre);
    if let Some(mine) = world.get_mut::<Enemy>(enemy) {
        mine.home = home.to_array();
    }
    Some(enemy)
}

/// What a restored enemy's spawn gave it beyond its prefab and its saved
/// state: its kind's body and its hold on rock ([`fit`]).
pub fn restored(world: &mut World, enemy: Entity) {
    fit(world, enemy);
    disarm_as_prefab(world, enemy);
    steering_as_prefab(world, enemy);
}

/// An older save's flyer record, which held no steering, takes its
/// prefab's, read from the prefab's data; its next intent step switches it
/// on or off.
fn steering_as_prefab(world: &mut World, enemy: Entity) {
    let Some(mine) = world.get::<Enemy>(enemy).copied() else {
        return;
    };
    if !info(world, mine.kind).flyer || world.has::<Steering>(enemy) {
        return;
    }
    let steering = world
        .get_resource::<PrefabRegistry>()
        .and_then(|prefabs| prefabs.get(prefab(mine.kind, mine.variant)))
        .and_then(|data| data.components.get("steering"))
        .and_then(|steering| Steering::deserialize(steering.clone()).ok());
    if let Some(steering) = steering {
        world.insert(enemy, steering);
    }
}

/// An older save's enemy record, whose health lacks its prefab's death
/// policy's `disarm`, takes it, read from the prefab's data.
fn disarm_as_prefab(world: &mut World, enemy: Entity) {
    let Some(mine) = world.get::<Enemy>(enemy) else {
        return;
    };
    let disarms = world
        .get_resource::<PrefabRegistry>()
        .and_then(|prefabs| prefabs.get(prefab(mine.kind, mine.variant)))
        .and_then(|data| data.components.get("health"))
        .and_then(|health| health.get("disarm"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if disarms
        && let Some(health) = world.get_mut::<Health>(enemy)
        && !health.disarms()
    {
        *health = health.clone().disarming();
    }
}

/// An enemy's body from its kind's data ([`shared::fit_body`]): a spawn's
/// and a load's. A walker's controller takes its filter (lava's layer
/// left out), its shove by its kind's mass and its [`WALK_STEP`] from its
/// prefab's data (S-99).
fn fit(world: &mut World, enemy: Entity) {
    let Some(kind) = world.get::<Enemy>(enemy).map(|enemy| enemy.kind) else {
        return;
    };
    shared::fit_body(world, enemy, kind.name());
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
        half_extents: body(world, kind).half,
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

/// Summons a plain enemy of `kind` with its body's centre at `at`
/// ([`summon_as`]).
pub fn summon(world: &mut World, kind: EnemyKind, at: Vec2) -> Option<Entity> {
    summon_as(world, kind, Variant::Plain, at)
}

/// Summons an enemy of `kind`'s `variant` with its body's centre at `at`,
/// at the run's difficulty, with a poof; past [`SUMMON_CAP`] summoned live,
/// the oldest goes, with its own.
pub fn summon_as(world: &mut World, kind: EnemyKind, variant: Variant, at: Vec2) -> Option<Entity> {
    let difficulty =
        crate::shell::run_meta(world).map_or(Difficulty::Normal, |meta| meta.difficulty);
    let corner = at - body(world, kind).centre;
    let enemy = spawn_record(world, kind, variant, corner)?;
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
/// gives each its kind's body ([`shared::fit_body`]), sets its home
/// ([`home`]) and gives it the difficulty's factor; its mind and its state
/// machine are its prefab's.
pub fn start(world: &mut World, difficulty: Difficulty) {
    let placed: Vec<(Entity, u64, EnemyKind)> = world
        .query::<(Entity, &Enemy, &Saved)>()
        .map(|(entity, enemy, id)| (entity, id.id().0, enemy.kind))
        .collect();
    for &(entity, _, kind) in &placed {
        shared::fit_body(world, entity, kind.name());
        let at = centre(world, entity).unwrap_or_default();
        let home = world
            .get::<Enemy>(entity)
            .map_or(at, |enemy| home(world, enemy.kind, at));
        if let Some(enemy) = world.get_mut::<Enemy>(entity) {
            enemy.home = home.to_array();
        }
        scale(world, entity, difficulty);
    }
    let mut placed: Vec<u64> = placed.into_iter().map(|(_, id, _)| id).collect();
    placed.sort_unstable();
    world.insert_resource(PlacedEnemies(placed));
    world.insert_resource(Throws::default());
}

/// The live enemies, alive and not dead, with their save IDs, in save-ID
/// order.
#[must_use]
pub fn live(world: &World) -> Vec<(u64, Entity)> {
    let mut live: Vec<(u64, Entity)> = world
        .query::<(Entity, &Enemy, &Saved, &Health)>()
        .filter(|(.., health)| !health.is_dead())
        .map(|(entity, _, id, _)| (id.id().0, entity))
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

/// The seconds the clip `clip` (its manifest ID, `ex10_` and all) plays,
/// its frames' durations summed: a one-shot tell's or throw's, which its
/// state runs on its own fixed-step clock (S-170); 0 for a clip the
/// registry lacks.
#[must_use]
pub fn clip_secs(world: &World, clip: &str) -> f32 {
    world
        .get_resource::<AnimationRegistry>()
        .and_then(|registry| registry.get(clip))
        .map_or(0.0, |data| data.total_duration_ms() as f32 / 1000.0)
}

/// Which way `enemy` faces along x, 1 right or -1 left: its prefab's
/// `facing`, from an intent it never has, so only [`turn`] turns it; none
/// without one.
#[must_use]
pub fn heading_of(world: &World, enemy: Entity) -> Option<f32> {
    let facing = world.get::<Facing>(enemy)?;
    Some(if facing.direction().x < 0.0 {
        -1.0
    } else {
        1.0
    })
}

/// Turns `enemy` to `heading` along x, 1 right or -1 left
/// (`Facing::face`); its facing holds it until the next turn.
pub fn turn(world: &mut World, enemy: Entity, heading: f32) {
    if let Some(facing) = world.get_mut::<Facing>(enemy) {
        facing.face(Vec2::new(heading, 0.0));
    }
}

/// An enemy's body centre.
#[must_use]
pub fn centre(world: &World, enemy: Entity) -> Option<Vec2> {
    let at = world.get::<Position>(enemy)?.0;
    let kind = world.get::<Enemy>(enemy)?.kind;
    Some(at + body(world, kind).centre)
}

/// `kind`'s body, as its data holds it now: its plain prefab's name's, the
/// armoured troll's the troll's.
#[must_use]
pub fn body(world: &World, kind: EnemyKind) -> &Body {
    shared::body(world, kind.name())
}
