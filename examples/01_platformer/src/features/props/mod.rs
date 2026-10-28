//! The props (01 (g)): pine crates and red powder kegs, dynamic boxes the
//! spells, the gun and the troll meet as they meet iron. A crate catches
//! fire, burns for [`BURN_SECS`] and falls in; once it has burned
//! [`SPREAD_AFTER`] it lights the crates and kegs within [`SPREAD_REACH`] of
//! its box, and at once the small balls within [`TOUCH_REACH`]. Cold, steam
//! and a black hole put it out, its fuel kept, and keep it from lighting
//! for [`DOUSE_SECS`]. Whatever lights a crate lights a keg's fuse; after
//! [`FUSE_SECS`], or at once on a hard impact, a bullet, lightning or a
//! blast, the keg blows through the saved cascade queue
//! (`CascadeKind::Detonation`), crediting whoever lit it. A frozen crate or
//! keg never lights: fire on it is thermal shock. The summon slot places
//! them, [`CRATE_CAP`] crates and [`KEG_CAP`] kegs at most, the map's own
//! besides ([`MapProp`]). A goblin's bomb is a lit keg in small: thrown on the world's
//! gravity, it rolls, its fuse turning with it, blows at [`BOMB_BLAST`]
//! after [`FUSE_SECS`], credited to its goblin, and fizzles out at once
//! when snuffed. A goblin's rock is a stone body that bounces and rolls and
//! stays, [`ROCK_CAP`] at most, the oldest crumbling. A crate hit hard
//! enough breaks into five planks ([`break_crate`], e01sfx), which burn as
//! it does for at most [`PLANK_BURN_SECS`] and fall in quietly,
//! [`PLANK_CAP`] at most, the oldest crumbling. The hollow's rubble stones
//! (e01hollow), three sizes the map sets down in heaps, are bodies as a
//! rock is that slide as a crate does, never burning or freezing, with no
//! cap, since none is summoned.

pub mod components;
pub mod draw;
pub mod systems;

use glam::Vec2;
use tungsten::core::{
    BodyKind, Entity, RigidBody, Schedule, Shape, Stage, Velocity, World, system,
};
use tungsten::kit::{Phase, ensure_phases};
use tungsten::physics::{SpatialFilter, overlap};
use tungsten::plugins::{PARTICLE_COUNT_REFRESH, PHYSICS_SYNC};
use tungsten::{Extracts, Plugin};

use crate::features::audio::{self, components::Sound};
use crate::features::elements::DOUSE_SECS;
use crate::features::elements::cascade::{self, CascadeKind};
use crate::features::elements::components::Frozen;
use crate::features::level::components::SceneTime;
use crate::features::toys::components::ScrapTumble;
use crate::features::toys::{self, components::IronBlock};
use crate::shared::{SaveId, body_centre, despawn_tree, spawn_burst, spawn_key_burst};
use components::{Bomb, Crate, Fuel, Keg, MapProp, Plank, PropKind, Rock, Stone};

/// The props' prefabs, by their manifest IDs.
pub const CRATE_PREFAB: &str = "crate";
pub const KEG_PREFAB: &str = "keg";
pub const BOMB_PREFAB: &str = "goblin_bomb";
pub const ROCK_PREFAB: &str = "goblin_rock";
pub const PLANK_PREFAB: &str = "plank";
pub const STONE_PREFABS: [&str; 3] = ["stone_small", "stone_medium", "stone_large"];
/// A crate's half size and its centre from its sprite's top-left, as its
/// prefab sets them, and a keg's.
pub const CRATE_HALF: Vec2 = Vec2::splat(28.0);
pub const CRATE_CENTRE: Vec2 = Vec2::splat(32.0);
pub const KEG_HALF: Vec2 = Vec2::new(19.0, 25.0);
pub const KEG_CENTRE: Vec2 = Vec2::new(32.0, 35.0);
/// A bomb's radius and its centre in its 16 px art, as its prefab sets
/// them, and a rock's: each the art's middle, which its sprite turns about.
pub const BOMB_RADIUS: f32 = 6.0;
pub const BOMB_CENTRE: Vec2 = Vec2::splat(8.0);
pub const ROCK_RADIUS: f32 = 5.0;
pub const ROCK_CENTRE: Vec2 = Vec2::splat(8.0);
/// A plank's half size and its centre from its 32 × 16 art's top-left, as
/// its prefab sets them.
pub const PLANK_HALF: Vec2 = Vec2::new(14.0, 4.0);
pub const PLANK_CENTRE: Vec2 = Vec2::new(16.0, 8.0);
/// Each stone size's half size and its centre from its art's top-left, as
/// its prefab sets them: a small and a medium stone on 32 px art, a large
/// one on 64, each box's bottom the art's lowest opaque row.
pub const STONE_HALVES: [Vec2; 3] = [
    Vec2::new(9.0, 7.0),
    Vec2::new(13.0, 10.0),
    Vec2::new(20.0, 15.0),
];
pub const STONE_CENTRES: [Vec2; 3] = [
    Vec2::new(16.0, 25.0),
    Vec2::new(16.0, 22.0),
    Vec2::new(32.0, 49.0),
];
/// Most summoned crates and most summoned kegs at once, the map's own
/// besides (Q45): one placed past its cap replaces the oldest summoned,
/// which poofs.
pub const CRATE_CAP: usize = 16;
pub const KEG_CAP: usize = 8;
/// Most rocks at once: a throw past it crumbles the oldest.
pub const ROCK_CAP: usize = 24;
/// Most planks at once: a break past it crumbles the oldest.
pub const PLANK_CAP: usize = 40;
/// A plank's fire at most, game seconds.
pub const PLANK_BURN_SECS: f32 = 4.0;
/// A broken crate's planks, one a row of its box from the top: each row's
/// height from the crate's centre; the least and the most speed a row is
/// kicked out at, away from the crate's centre; the lift each takes; and
/// the share of the hitter's velocity each carries on.
pub const PLANK_ROWS: [f32; 5] = [-20.0, -10.0, 0.0, 10.0, 20.0];
pub const PLANK_KICK_LEAST: f32 = 260.0;
pub const PLANK_KICK_MOST: f32 = 420.0;
pub const PLANK_LIFT: f32 = 180.0;
pub const PLANK_CARRY: f32 = 0.25;
/// A crate's fire, game seconds; the seconds it burns before it lights the
/// crates and kegs within [`SPREAD_REACH`] of its box; and how near its box
/// a small ball lights from it, or a burning small ball lights it.
pub const BURN_SECS: f32 = 8.0;
pub const SPREAD_AFTER: f32 = 2.0;
pub const SPREAD_REACH: f32 = 4.0;
pub const TOUCH_REACH: f32 = 2.0;
/// A keg's fuse, game seconds, a bomb's too, and their blasts' reaches.
pub const FUSE_SECS: f32 = 1.5;
pub const KEG_BLAST: f32 = 192.0;
pub const BOMB_BLAST: f32 = 128.0;
/// A frozen crate's or keg's thermal shock reach.
pub const PROP_BLAST: f32 = 96.0;
/// The fire a burning crate's impact adds to its hit on a live target.
pub const BURNING_IMPACT: f32 = 12.0;
/// Wood's sliding friction while it rests on something, px/s², and the
/// share of a hole's pull a crate and a keg take.
pub const WOOD_FRICTION: f32 = 900.0;
/// A rolling bomb's or rock's slowing while it rests on something, px/s².
pub const ROLL_FRICTION: f32 = 300.0;
pub const CRATE_PULL: f32 = 0.5;
pub const KEG_PULL: f32 = 0.4;

/// The props as a plugin: the `crate`, `keg`, `goblin_bomb` and `rock`
/// components their prefabs name; in the fixed step, the fires', the
/// fuses' and the douses' clocks and the round props' roll in the tick
/// phase, and after the physics step, before the cascades' run, the heat
/// between them, the burning balls and the lava; in the present phase their
/// looks, and before the frame's particles a lit fuse's emitters; and their
/// glows, chars, flames and ice drawn after the sprites.
pub struct PropsPlugin;

impl Plugin for PropsPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        // Before the manifest loads, which checks each prefab's components.
        world.register_component::<Crate>("crate");
        world.register_component::<Keg>("keg");
        world.register_component::<Bomb>("goblin_bomb");
        world.register_component::<Rock>("rock");
        world.register_component::<Plank>("plank");
        world.register_component::<Stone>("stone");
        world.resource_mut::<Extracts>().add_sprites(draw::effects);
        ensure_phases(schedule);
        let tick = system("props_tick", systems::tick);
        schedule.add(Stage::FixedUpdate, Phase::Tick.join(tick));
        let roll = system("props_roll", systems::roll);
        schedule.add(Stage::FixedUpdate, Phase::Tick.join(roll));
        let heat = system("props_heat", systems::heat).before("cascade_run");
        schedule.add(Stage::FixedUpdate, Phase::Overlap.join(heat));
        let look = system("props_look", systems::look);
        schedule.add(Stage::PostUpdate, Phase::Present.join(look));
        // Before the frame's emission, which they place.
        let fuses = system("fuse_emitters", systems::fuses)
            .after(PHYSICS_SYNC)
            .before(PARTICLE_COUNT_REFRESH);
        schedule.add(Stage::PostUpdate, fuses);
    }
}

/// What `entity` is, a crate, a keg, a bomb, a rock, a plank or a stone;
/// None for anything else.
#[must_use]
pub fn kind(world: &World, entity: Entity) -> Option<PropKind> {
    if world.has::<Crate>(entity) {
        Some(PropKind::Crate)
    } else if world.has::<Keg>(entity) {
        Some(PropKind::Keg)
    } else if world.has::<Bomb>(entity) {
        Some(PropKind::Bomb)
    } else if world.has::<Rock>(entity) {
        Some(PropKind::Rock)
    } else if world.has::<Plank>(entity) {
        Some(PropKind::Plank)
    } else {
        world
            .get::<Stone>(entity)
            .map(|stone| PropKind::Stone(stone.size))
    }
}

/// Whether `entity` is a crate, a keg, a bomb, a rock, a plank or a stone.
#[must_use]
pub fn is_prop(world: &World, entity: Entity) -> bool {
    kind(world, entity).is_some()
}

/// Every crate, keg, bomb, rock, plank and stone, from their own columns:
/// a pass over the props never walks the balls (P6).
#[must_use]
pub fn all(world: &World) -> Vec<Entity> {
    world
        .query::<(Entity, &Crate)>()
        .map(|(entity, _)| entity)
        .chain(world.query::<(Entity, &Keg)>().map(|(entity, _)| entity))
        .chain(world.query::<(Entity, &Bomb)>().map(|(entity, _)| entity))
        .chain(world.query::<(Entity, &Rock)>().map(|(entity, _)| entity))
        .chain(world.query::<(Entity, &Plank)>().map(|(entity, _)| entity))
        .chain(world.query::<(Entity, &Stone)>().map(|(entity, _)| entity))
        .collect()
}

/// A kind's half size: a bomb's or a rock's, its circle's.
#[must_use]
pub const fn half(kind: PropKind) -> Vec2 {
    match kind {
        PropKind::Crate => CRATE_HALF,
        PropKind::Keg => KEG_HALF,
        PropKind::Bomb => Vec2::splat(BOMB_RADIUS),
        PropKind::Rock => Vec2::splat(ROCK_RADIUS),
        PropKind::Plank => PLANK_HALF,
        PropKind::Stone(size) => STONE_HALVES[size as usize],
    }
}

/// A kind's body centre from its sprite's top-left.
#[must_use]
pub const fn centre(kind: PropKind) -> Vec2 {
    match kind {
        PropKind::Crate => CRATE_CENTRE,
        PropKind::Keg => KEG_CENTRE,
        PropKind::Bomb => BOMB_CENTRE,
        PropKind::Rock => ROCK_CENTRE,
        PropKind::Plank => PLANK_CENTRE,
        PropKind::Stone(size) => STONE_CENTRES[size as usize],
    }
}

/// A kind's prefab.
#[must_use]
pub const fn prefab(kind: PropKind) -> &'static str {
    match kind {
        PropKind::Crate => CRATE_PREFAB,
        PropKind::Keg => KEG_PREFAB,
        PropKind::Bomb => BOMB_PREFAB,
        PropKind::Rock => ROCK_PREFAB,
        PropKind::Plank => PLANK_PREFAB,
        PropKind::Stone(size) => STONE_PREFABS[size as usize],
    }
}

/// A kind's cap: bombs have none, bounded by the goblins' pace, and stones
/// none, the map's own and never summoned.
#[must_use]
pub const fn cap(kind: PropKind) -> usize {
    match kind {
        PropKind::Crate => CRATE_CAP,
        PropKind::Keg => KEG_CAP,
        PropKind::Bomb => usize::MAX,
        PropKind::Rock => ROCK_CAP,
        PropKind::Plank => PLANK_CAP,
        PropKind::Stone(_) => usize::MAX,
    }
}

/// Whether a kind rolls: a round bomb or rock, which turns as it travels
/// and slows by [`ROLL_FRICTION`] where wood slows by [`WOOD_FRICTION`].
#[must_use]
pub const fn rolls(kind: PropKind) -> bool {
    matches!(kind, PropKind::Bomb | PropKind::Rock)
}

/// Whether a prop of `kind` fits with its centre at `at`, as an iron block
/// fits (`toys::iron_fits`): no solid tile, no body that is not dynamic,
/// and no iron block or other prop in its box, at the last step's places.
#[must_use]
pub fn fits(world: &World, kind: PropKind, at: Vec2) -> bool {
    let mut met = Vec::new();
    let shape = Shape::Aabb {
        half_extents: half(kind),
    };
    overlap(world, shape, at, &SpatialFilter::new(), &mut met);
    met.iter().all(|hit| {
        hit.is_some_and(|entity| {
            !world.has::<IronBlock>(entity)
                && !is_prop(world, entity)
                && world
                    .get::<RigidBody>(entity)
                    .is_some_and(|body| body.kind == BodyKind::Dynamic)
        })
    })
}

/// Spawns a prop of `kind` with its body's centre at `at`, at once and
/// quietly, with the run's next save ID: a crate's, a rock's or a stone's
/// art by the ID's parity ([`art`]), a crate's fuel whole; a plank its
/// first art and its fuel whole, which a break sets.
pub fn spawn(world: &mut World, kind: PropKind, at: Vec2) -> Entity {
    let prop = toys::spawn_toy(world, prefab(kind), at - centre(kind), 1.0);
    let variant = art(world.get::<SaveId>(prop));
    if kind == PropKind::Rock {
        world.insert(prop, Rock { variant });
        systems::show_rock(world, prop, variant);
    }
    if kind == PropKind::Crate {
        world.insert(prop, Crate { variant });
        world.insert(
            prop,
            Fuel {
                left: BURN_SECS,
                lit: false,
                lighter: None,
                doused: 0.0,
            },
        );
        systems::show_variant(world, prop, variant);
    }
    if let PropKind::Stone(size) = kind {
        world.insert(prop, Stone { size, variant });
        systems::show_stone(world, prop, size, variant);
    }
    if kind == PropKind::Plank {
        world.insert(prop, Plank { variant: 0 });
        world.insert(
            prop,
            Fuel {
                left: PLANK_BURN_SECS,
                lit: false,
                lighter: None,
                doused: 0.0,
            },
        );
    }
    prop
}

/// A saved prop of `kind` set back with its point at `at`, the top-left of
/// its art, under its save `id`, at once and quietly: a load's, which puts
/// its state over it. A crate, a rock, a plank or a stone takes its art's
/// `variant`.
pub fn restore(world: &mut World, kind: PropKind, at: Vec2, id: SaveId, variant: usize) -> Entity {
    let prop = toys::spawn_toy(world, prefab(kind), at, 1.0);
    world.insert(prop, id);
    match kind {
        PropKind::Crate => {
            world.insert(prop, Crate { variant });
            systems::show_variant(world, prop, variant);
        }
        PropKind::Rock => {
            world.insert(prop, Rock { variant });
            systems::show_rock(world, prop, variant);
        }
        PropKind::Plank => {
            world.insert(prop, Plank { variant });
            systems::show_plank(world, prop, variant);
        }
        PropKind::Stone(size) => {
            world.insert(prop, Stone { size, variant });
            systems::show_stone(world, prop, size, variant);
        }
        PropKind::Keg | PropKind::Bomb => {}
    }
    prop
}

/// Breaks `crate_` under a heavy hit (V33): it goes with its splinters, a
/// key burst, and its crack, and five planks take its place, one a row of
/// its box from the top ([`PLANK_ROWS`]), each at the crate's velocity
/// plus a kick away from its centre, [`PLANK_KICK_LEAST`] to
/// [`PLANK_KICK_MOST`] by row and to its side, less [`PLANK_LIFT`] up,
/// plus [`PLANK_CARRY`] of `hitter`'s velocity, tumbling whole half-turns
/// as it flies so it lies flat. A burning crate's planks burn with its fire
/// left, at most [`PLANK_BURN_SECS`], crediting its lighter; a doused one's
/// keep the douse. Past [`PLANK_CAP`] the oldest crumble first.
pub fn break_crate(world: &mut World, crate_: Entity, hitter: Vec2) {
    let Some(at) = body_centre(world, crate_) else {
        return;
    };
    let velocity = world.get::<Velocity>(crate_).map_or(Vec2::ZERO, |v| v.0);
    let fuel = world.get::<Fuel>(crate_).copied();
    let born_at = world.get_resource::<SceneTime>().map_or(0.0, |time| time.0);
    spawn_key_burst(world, "ex10_plank_splinters", at);
    audio::play(world, Sound::CrateBroke, at);
    despawn_tree(world, crate_);
    let mut planks: Vec<(SaveId, Entity)> = world
        .query::<(Entity, &Plank, &SaveId)>()
        .map(|(entity, _, id)| (*id, entity))
        .collect();
    planks.sort_by_key(|(id, _)| *id);
    let surplus = (planks.len() + PLANK_ROWS.len()).saturating_sub(PLANK_CAP);
    for &(_, oldest) in planks.iter().take(surplus) {
        crumble_plank(world, oldest);
    }
    let lowest = PLANK_ROWS[PLANK_ROWS.len() - 1];
    for (row, &height) in PLANK_ROWS.iter().enumerate() {
        let side = if row % 2 == 0 { -1.0 } else { 1.0 };
        // Permutes the five kicks without the game's stream.
        let share = (row * 3 % PLANK_ROWS.len()) as f32 / (PLANK_ROWS.len() - 1) as f32;
        let speed = PLANK_KICK_LEAST + (PLANK_KICK_MOST - PLANK_KICK_LEAST) * share;
        let away = Vec2::new(side, height / lowest).normalize();
        let kick = away * speed - Vec2::Y * PLANK_LIFT;
        let plank = spawn(world, PropKind::Plank, at + Vec2::new(0.0, height));
        world.insert(plank, Plank { variant: row });
        systems::show_plank(world, plank, row);
        if let Some(own) = world.get_mut::<Velocity>(plank) {
            own.0 = velocity + kick + hitter * PLANK_CARRY;
        }
        let tumble = ScrapTumble {
            born_at,
            initial_angle: 0.0,
            total_turn: side * (1 + row % 2) as f32 * std::f32::consts::PI,
        };
        let burn = Fuel {
            left: fuel.map_or(PLANK_BURN_SECS, |fuel| fuel.left.min(PLANK_BURN_SECS)),
            lit: fuel.is_some_and(|fuel| fuel.lit),
            lighter: fuel.and_then(|fuel| fuel.lighter),
            doused: fuel.map_or(0.0, |fuel| fuel.doused),
        };
        world.insert_bundle(plank, (tumble, burn));
    }
}

/// A plank past [`PLANK_CAP`] crumbling where it is, quietly: its
/// splinters, a key burst.
pub fn crumble_plank(world: &mut World, plank: Entity) {
    if let Some(at) = body_centre(world, plank) {
        spawn_key_burst(world, "ex10_plank_splinters", at);
    }
    despawn_tree(world, plank);
}

/// A goblin's rock with its centre at `at`, flying at `velocity`, at once,
/// with the run's next save ID; past [`ROCK_CAP`] the oldest, the lowest
/// save ID, crumbles first. The thrower credits it through `Throws`.
pub fn lob_rock(world: &mut World, at: Vec2, velocity: Vec2) -> Entity {
    let mut rocks: Vec<(SaveId, Entity)> = world
        .query::<(Entity, &Rock, &SaveId)>()
        .map(|(entity, _, id)| (*id, entity))
        .collect();
    rocks.sort_by_key(|(id, _)| *id);
    let surplus = (rocks.len() + 1).saturating_sub(ROCK_CAP);
    for &(_, oldest) in rocks.iter().take(surplus) {
        crumble(world, oldest);
    }
    let rock = spawn(world, PropKind::Rock, at);
    if let Some(own) = world.get_mut::<tungsten::core::Velocity>(rock) {
        own.0 = velocity;
    }
    rock
}

/// A rock breaking up where it is: its chips, a key burst, and its clack.
pub fn crumble(world: &mut World, rock: Entity) {
    if let Some(at) = body_centre(world, rock) {
        spawn_key_burst(world, "ex10_rock_chips", at);
        audio::play(world, Sound::RockHit, at);
    }
    despawn_tree(world, rock);
}

/// A crate's, a rock's or a stone's art by its save ID's parity, an odd
/// ID's the first: a fresh run's runtime IDs count from 0 where schema 22's
/// counted from 57, past the map's, so the map's crates and stones keep
/// the art they were drawn and placed with.
#[must_use]
pub fn art(id: Option<&SaveId>) -> usize {
    id.map_or(0, |id| ((id.0 + 1) % 2) as usize)
}

/// Places a prop of `kind` with its centre at `at`, as the summon slot
/// does: past its kind's cap, the map's own aside ([`MapProp`]), the
/// oldest, the lowest save ID, goes first in a poof.
pub fn place(world: &mut World, kind: PropKind, at: Vec2) -> Entity {
    let mut live: Vec<(SaveId, Entity)> = all(world)
        .into_iter()
        .filter(|&entity| self::kind(world, entity) == Some(kind))
        .filter(|&entity| !world.has::<MapProp>(entity))
        .filter_map(|entity| Some((*world.get::<SaveId>(entity)?, entity)))
        .collect();
    live.sort_by_key(|(id, _)| *id);
    let surplus = (live.len() + 1).saturating_sub(cap(kind));
    for &(_, oldest) in live.iter().take(surplus) {
        if let Some(at) = body_centre(world, oldest) {
            spawn_key_burst(world, "ex10_poof", at);
        }
        despawn_tree(world, oldest);
    }
    spawn(world, kind, at)
}

/// Whether `prop` burns: a crate on fire, or a keg's or a bomb's lit fuse.
#[must_use]
pub fn burning(world: &World, prop: Entity) -> bool {
    world.get::<Fuel>(prop).is_some_and(|fuel| fuel.lit)
        || world.get::<Keg>(prop).is_some_and(|keg| keg.fuse.is_some())
        || world
            .get::<Bomb>(prop)
            .is_some_and(|bomb| bomb.fuse.is_some())
}

/// Spawns a goblin's bomb with its centre at `at`, lit for [`FUSE_SECS`]
/// and credited to the save ID `lighter`, at once, with the run's next save
/// ID: a throw's, which sets its velocity.
pub fn spawn_bomb(world: &mut World, at: Vec2, lighter: Option<u64>) -> Entity {
    let bomb = toys::spawn_toy(world, BOMB_PREFAB, at - BOMB_CENTRE, 1.0);
    world.insert(
        bomb,
        Bomb {
            fuse: Some(FUSE_SECS),
            lighter,
        },
    );
    if let Some(at) = body_centre(world, bomb) {
        audio::play(world, Sound::FuseLit, at);
    }
    bomb
}

/// Lights `prop`, its fire credited to the save ID `lighter`: a crate
/// catches with its burst and its sound, a keg's fuse starts for
/// [`FUSE_SECS`] with its hiss. Neither lights while frozen, doused or
/// alight, nor a crate burnt out. Whether it lit.
pub fn light(world: &mut World, prop: Entity, lighter: Option<u64>) -> bool {
    if world.has::<Frozen>(prop) {
        return false;
    }
    let Some(at) = body_centre(world, prop) else {
        return false;
    };
    if let Some(fuel) = world.get_mut::<Fuel>(prop) {
        if fuel.lit || fuel.doused > 0.0 || fuel.left <= 0.0 {
            return false;
        }
        fuel.lit = true;
        fuel.lighter = lighter;
        spawn_burst(world, "ex10_crate_ignite", at);
        audio::play(world, Sound::CrateIgnited, at);
        return true;
    }
    if let Some(keg) = world.get_mut::<Keg>(prop) {
        if keg.fuse.is_some() || keg.doused > 0.0 {
            return false;
        }
        keg.fuse = Some(FUSE_SECS);
        keg.lighter = lighter;
        audio::play(world, Sound::FuseLit, at);
        return true;
    }
    false
}

/// Sets `keg`, a keg or a bomb, off at once, its blast credited to the save
/// ID `lighter`, in the chain begun at `origin` (its own centre when none):
/// its fuse spent and its detonation queued, due at the step's run when
/// queued before it. A frozen keg never blows. Whether it queued.
pub fn set_off(world: &mut World, keg: Entity, origin: Option<Vec2>, lighter: Option<u64>) -> bool {
    if world.has::<Frozen>(keg) {
        return false;
    }
    let Some(at) = body_centre(world, keg) else {
        return false;
    };
    let reach = if let Some(state) = world.get_mut::<Keg>(keg) {
        state.fuse = None;
        KEG_BLAST
    } else if let Some(state) = world.get_mut::<Bomb>(keg) {
        state.fuse = None;
        BOMB_BLAST
    } else {
        return false;
    };
    cascade::queue_from(
        world,
        CascadeKind::Detonation,
        Some(keg),
        at,
        origin.unwrap_or(at),
        reach,
        lighter,
    )
}

/// Puts `prop` out, as cold, steam or a black hole does: a crate's fire,
/// its fuel left kept, or a keg's lit fuse, either kept from lighting for
/// [`DOUSE_SECS`]; a bomb fizzles out at once. Whether it burned.
pub fn put_out(world: &mut World, prop: Entity) -> bool {
    if world.has::<Bomb>(prop) {
        if let Some(at) = body_centre(world, prop) {
            spawn_key_burst(world, "ex10_fizzle", at);
            audio::play(world, Sound::Fizzled, at);
        }
        despawn_tree(world, prop);
        return true;
    }
    if let Some(fuel) = world.get_mut::<Fuel>(prop) {
        let burned = fuel.lit;
        fuel.lit = false;
        fuel.doused = DOUSE_SECS;
        return burned;
    }
    if let Some(keg) = world.get_mut::<Keg>(prop) {
        let burned = keg.fuse.is_some();
        keg.fuse = None;
        keg.lighter = None;
        keg.doused = DOUSE_SECS;
        return burned;
    }
    false
}

/// A plank burnt out falls in where it is, quietly: its embers, a key
/// burst.
pub fn burn_down(world: &mut World, plank: Entity) {
    if let Some(at) = body_centre(world, plank) {
        spawn_key_burst(world, "ex10_collapse_embers", at);
    }
    despawn_tree(world, plank);
}

/// A crate burnt out falls in where it is, from a troll's hands too: its
/// smoke, its embers and its charred splinters, key bursts, and its sound.
pub fn collapse(world: &mut World, prop: Entity) {
    if let Some(at) = body_centre(world, prop) {
        spawn_key_burst(world, "ex10_burn_out", at + Vec2::new(0.0, 8.0));
        spawn_key_burst(world, "ex10_collapse_embers", at + Vec2::new(0.0, 12.0));
        spawn_key_burst(world, "ex10_crate_collapse", at + Vec2::new(0.0, 12.0));
        audio::play(world, Sound::CrateCollapsed, at);
    }
    despawn_tree(world, prop);
}
