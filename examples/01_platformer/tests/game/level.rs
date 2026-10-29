//! The level from its generator, on the kit's controller and path follower:
//! every route's jumps, each tome's support by the double jump, a ride
//! through a platform's cycle, the platforms'
//! sweeps, the elevator turning back over a player and setting one out of
//! its deck, a still rider idling on it, a thin deck's underside, one
//! collider per slab and iron box, and the start's wall and boundary
//! keeping a thrown ball in the map.

use std::collections::{BTreeMap, BTreeSet};

use example_01_platformer::features::level::components::Hazard;
use example_01_platformer::features::level::elevator::{Elevator, Stop};
use example_01_platformer::features::level::{BOUNDARY, LAVA_LAYER, MAP, TILE};
use example_01_platformer::features::player;
use example_01_platformer::features::run::Run;
use glam::Vec2;
use serde::Deserialize;
use tungsten::core::{
    Collider, Entity, LayerKind, PhysicsConfig, Position, PrevPosition, RigidBody, Shape, Sprite,
    TilemapRegistry, Velocity,
};
use tungsten::kit::{CharacterAnimator, PathFollower, PlatformerController};
use tungsten::physics::BodyKind;
use tungsten::testing::UiHarness;

use super::player_half;
use super::{ball, calm, harness, start};

const DT: f32 = 1.0 / 60.0;

/// `tools/level.json`, the generator's source, as far as these tests read it.
#[derive(Deserialize)]
struct Source {
    platforms: Vec<Platform>,
    routes: BTreeMap<String, Vec<String>>,
    hazards: Vec<SourceHazard>,
    moving_platforms: Vec<Moving>,
    kill_row: f32,
    tomes: Vec<SourceTome>,
}

/// A book: its name, the platform it lies over and the route platform its
/// launch starts on.
#[derive(Deserialize)]
struct SourceTome {
    name: String,
    support: String,
    from: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Platform {
    name: String,
    left: f32,
    right: f32,
    row: f32,
}

impl Source {
    fn platform(&self, name: &str) -> &Platform {
        self.platforms
            .iter()
            .find(|p| p.name == name)
            .unwrap_or_else(|| panic!("no platform {name}"))
    }
}

/// The deck the underside test reads: a wooden one over the ball pit, with
/// open air under it.
const UNDERSIDE_DECK: &str = "bridge_b";

#[derive(Deserialize)]
struct SourceHazard {
    fire: bool,
    position: [f32; 2],
}

#[derive(Deserialize)]
struct Moving {
    position: [f32; 2],
    #[serde(default)]
    travel: [f32; 2],
    half_width: f32,
    /// The mine's elevator: rows it rises from `position`, the row its
    /// floor's top rests at.
    #[serde(default)]
    elevator: Option<Lift>,
}

#[derive(Deserialize)]
struct Lift {
    rise: f32,
}

fn source() -> Source {
    let text = std::fs::read_to_string("tools/level.json").expect("tools/level.json reads");
    serde_json::from_str(&text).expect("tools/level.json parses")
}

/// A moving deck's depth, in pixels: the lift deck's art.
const DECK_DEPTH: f32 = 23.0;
/// The spikes' touch box's half extents.
const SPIKES_HALF: Vec2 = Vec2::new(27.0, 14.0);

/// The launch that lands each route pair: the platforms' names, start and
/// goal x in tiles, and whether the player jumps on the first step. Every
/// pair a route takes has one row, and a row no route takes fails, so the
/// table never keeps dead data.
const LAUNCHES: &[(&str, &str, f32, f32, bool)] = &[
    ("clearing", "terrace_a", 15.85, 16.75, true),
    ("terrace_a", "terrace_b", 18.35, 23.25, true),
    ("terrace_b", "meadow", 23.85, 24.75, true),
    ("meadow", "bridge_a", 47.85, 51.75, true),
    ("bridge_a", "bridge_b", 57.85, 62.75, true),
    ("east_rim", "aqueduct_step", 91.85, 92.75, true),
    ("aqueduct_step", "aqueduct_floor", 94.85, 95.75, true),
    ("aqueduct_floor", "ravine_step_1", 157.85, 158.75, true),
    ("ravine_step_1", "ravine_step_2", 165.85, 166.75, true),
    ("ravine_step_2", "ravine_step_3", 169.85, 170.75, true),
    ("ravine_step_3", "ravine_step_4", 173.85, 174.75, true),
    ("ravine_step_4", "pool_bed", 176.85, 177.75, true),
    ("pool_bed", "cleft", 205.85, 206.75, true),
    ("cleft", "hollow_floor_1", 215.85, 216.75, true),
    ("hollow_floor_1", "stepping_stone", 229.85, 234.75, true),
    ("stepping_stone", "hollow_floor_2", 235.85, 240.75, true),
    ("hollow_floor_2", "hollow_step", 249.85, 250.75, true),
    ("hollow_step", "hollow_shelf_a", 253.85, 254.75, true),
    ("hollow_shelf_a", "hollow_shelf_b", 259.85, 263.75, true),
    ("hollow_shelf_b", "gallery", 267.85, 268.75, true),
    ("gallery", "cut_floor_1", 282.85, 283.75, true),
    ("cut_floor_1", "cut_floor_2", 305.35, 306.75, true),
    ("cut_floor_2", "cut_floor_2b", 314.85, 318.75, true),
    ("cut_floor_2b", "cut_floor_3", 323.35, 324.75, true),
    ("cut_floor_3", "cut_step_1", 341.35, 342.75, true),
    ("cut_step_1", "cut_step_2", 344.35, 345.75, true),
    ("cut_step_2", "cut_step_3", 347.35, 348.75, true),
    ("cut_step_3", "ridge", 350.85, 351.75, true),
    ("ridge", "bridge_head", 359.85, 360.75, true),
    ("bridge_head", "troll_bridge", 365.85, 366.75, true),
    ("troll_bridge", "east_bank", 423.85, 424.75, true),
    ("terrace_a", "forest_perch", 16.35, 12.75, true),
    ("forest_perch", "forest_shrine", 12.35, 8.75, true),
    ("pit_floor", "pit_shelf_1", 57.85, 55.75, true),
    ("pit_shelf_1", "pit_shelf_2", 56.85, 59.75, true),
    ("pit_shelf_2", "pit_shelf_3", 61.35, 63.75, true),
    ("pit_shelf_3", "pit_shelf_4", 63.35, 59.75, true),
    ("pit_shelf_4", "pit_shelf_5", 59.35, 55.75, true),
    ("pit_shelf_5", "pit_shelf_6", 56.85, 59.75, true),
    ("pool_bed", "ravine_east_1", 198.85, 200.75, true),
    ("ravine_east_1", "ravine_east_2", 201.35, 197.75, true),
    ("ravine_east_2", "ravine_east_3", 198.85, 200.75, true),
    ("ravine_east_3", "ravine_east_4", 201.35, 197.75, true),
    ("ravine_east_4", "ravine_east_5", 198.85, 200.75, true),
    ("cut_floor_1", "scaffold_1", 297.35, 293.75, true),
    ("scaffold_1", "scaffold_2", 294.35, 290.75, true),
    ("scaffold_2", "scaffold_3", 291.35, 293.75, true),
    ("scaffold_3", "cut_top", 293.35, 281.75, true),
    ("aqueduct_floor", "aq_rubble_1", 114.85, 112.75, true),
    ("aq_rubble_1", "aqueduct_floor", 114.85, 95.75, true),
    ("aqueduct_floor", "aq_rubble_2", 123.85, 121.75, true),
    ("aq_rubble_2", "aqueduct_floor", 123.85, 95.75, true),
    ("bridge_b", "east_rim", 68.85, 72.75, true),
    ("aqueduct_step", "aq_ledge_1", 94.85, 97.75, true),
    ("aq_ledge_1", "aq_ledge_2", 100.85, 103.75, true),
    ("aq_ledge_2", "aq_ledge_3", 106.85, 109.75, true),
    ("aq_ledge_3", "channel_mid", 111.85, 113.75, true),
    ("channel_mid", "channel_east_a", 128.85, 133.75, true),
    ("channel_east_a", "channel_east_b", 144.85, 149.75, true),
    ("channel_east_b", "rv_ledge_1", 177.85, 181.75, true),
    ("rv_ledge_1", "rv_ledge_2", 184.85, 187.75, true),
    ("rv_ledge_2", "rv_ledge_3", 187.35, 182.75, true),
    ("rv_ledge_3", "rv_ledge_4", 185.85, 188.75, true),
    ("rv_ledge_4", "rv_ledge_5", 188.35, 182.75, true),
    ("rv_ledge_5", "rv_ledge_6", 185.85, 188.75, true),
    ("rv_ledge_6", "rv_ledge_7", 188.35, 183.75, true),
    ("rv_ledge_7", "pool_bed", 186.85, 177.75, true),
    ("pit_shelf_6", "pit_shelf_7", 61.35, 63.75, true),
    ("pit_shelf_7", "pit_shelf_8", 63.35, 59.75, true),
    ("pit_shelf_8", "pit_shelf_9", 59.35, 55.75, true),
    ("pit_shelf_9", "pit_shelf_10", 56.85, 59.75, true),
    ("pit_shelf_10", "bridge_a", 59.35, 51.75, true),
    ("east_rim", "pit_floor", 72.35, 48.75, false),
    ("meadow", "pit_floor", 47.85, 48.75, true),
];

/// The launch that lands each book's support from its `from` by the
/// double jump: the book's name in `tools/level.json`'s `tomes`, start and
/// goal x in tiles. Every book has one row, and a row no book takes fails.
const TOME_LAUNCHES: &[(&str, f32, f32)] = &[
    ("pact tome", 226.85, 231.75),
    ("bounding tome", 191.85, 194.75),
    ("arcane tome", 124.35, 117.75),
];

/// A run with no hazard or moving platform, so a route's jumps meet the map
/// alone, at a fixed step a frame.
fn bare_level() -> UiHarness {
    let mut harness = harness();
    start(&mut harness);
    calm(&mut harness);
    harness.harness_mut().set_dt(DT);
    let world = harness.world_mut();
    let movers: Vec<Entity> = world
        .query::<(Entity, Option<&Hazard>, Option<&PathFollower>)>()
        .filter(|(_, hazard, path)| hazard.is_some() || path.is_some())
        .map(|(entity, ..)| entity)
        .collect();
    for entity in movers {
        world.despawn(entity);
    }
    harness
}

/// Replaces the run's player with a fresh one whose body's centre is
/// `centre`: its controller takes its still first step again.
fn fresh_player(harness: &mut UiHarness, centre: Vec2) -> Entity {
    let world = harness.world_mut();
    let old = world.resource::<Run>().player;
    world.despawn(old);
    let player = player::spawn(world, centre);
    world.resource_mut::<Run>().player = player;
    player
}

fn centre(harness: &UiHarness, player: Entity) -> Vec2 {
    player::centre(harness.world(), player).expect("the player has a body")
}

fn grounded(harness: &UiHarness, player: Entity) -> bool {
    let controller = harness.world().get::<PlatformerController>(player);
    controller.is_some_and(PlatformerController::grounded)
}

fn release_all(harness: &mut UiHarness) {
    for action in ["move_left", "move_right", "jump"] {
        harness.harness_mut().release_action(action);
    }
}

/// Whether a player's body centred at `centre` meets a collision tile.
fn embedded(harness: &UiHarness, centre: Vec2) -> bool {
    let map = harness
        .world()
        .resource::<TilemapRegistry>()
        .get(MAP)
        .expect("the level's map");
    let collision = map
        .layers
        .iter()
        .find(|layer| layer.kind == LayerKind::Collision)
        .expect("a collision layer");
    let (min, max) = (centre - player_half(), centre + player_half());
    let rows = (min.y / TILE).floor() as u32..(max.y / TILE).ceil() as u32;
    rows.flat_map(|row| {
        ((min.x / TILE).floor() as u32..(max.x / TILE).ceil() as u32).map(move |col| (col, row))
    })
    .any(|(col, row)| collision.tiles[(row * map.width + col) as usize] >= 0)
}

/// One attempt with ordinary input on the real map: the player settles at
/// `start` on `from`, steers toward `goal`, holding jump through the rise
/// when it jumps so the jump cut never acts, and must stand on `to`. With
/// `air` it releases jump at the rise's top and presses it again the next
/// step, so the controller makes its air jump.
fn launch_lands(
    harness: &mut UiHarness,
    level: &Source,
    from: &Platform,
    to: &Platform,
    (start, goal, jump): (f32, f32, bool),
    air: bool,
) -> bool {
    // A stale row's start may lie off `from`; one on `to` would land at
    // once. A book's start may not meet a solid stair over `from`, as one
    // inside `aq_rubble_2` did; the routes' rows keep their old starts,
    // some of which overhang the next step's face.
    let at = Vec2::new(start * TILE, from.row * TILE - player_half().y - 0.5);
    if !(from.left..from.right).contains(&start) || (air && embedded(harness, at)) {
        return false;
    }
    release_all(harness);
    let player = fresh_player(harness, at);
    harness.step(4);
    let settled = centre(harness, player);
    if !grounded(harness, player) || (settled.y + player_half().y - from.row * TILE).abs() > 1.0 {
        return false;
    }
    let spikes: Vec<Vec2> = level
        .hazards
        .iter()
        .filter(|hazard| !hazard.fire)
        .map(|hazard| Vec2::from(hazard.position) * TILE)
        .collect();
    // While rising, then released, then spent.
    let mut double = air.then_some(false);
    let landed = (0..(2.5 / DT) as usize).any(|tick| {
        let dx = goal * TILE - centre(harness, player).x;
        let falling = harness
            .world()
            .get::<Velocity>(player)
            .is_some_and(|velocity| velocity.0.y >= 0.0);
        let input = harness.harness_mut();
        input.release_action("move_left");
        input.release_action("move_right");
        if dx.abs() > 560.0 * DT * 0.6 {
            input.press_action(if dx > 0.0 { "move_right" } else { "move_left" });
        }
        if tick == 0 && jump {
            input.press_action("jump");
        }
        match double {
            Some(false) if tick > 0 && falling => {
                input.release_action("jump");
                double = Some(true);
            }
            Some(true) => {
                input.press_action("jump");
                double = None;
            }
            _ => {}
        }
        harness.step(1);
        let at = centre(harness, player);
        let spiked = spikes.iter().any(|spike| {
            let reach = player_half() + SPIKES_HALF;
            (at - *spike).abs().cmplt(reach).all()
        });
        if spiked || at.y > level.kill_row * TILE {
            return true;
        }
        tick > 3
            && grounded(harness, player)
            && (at.y + player_half().y - to.row * TILE).abs() < 1.0
            && at.x >= to.left * TILE + player_half().x
            && at.x <= to.right * TILE - player_half().x
    });
    release_all(harness);
    // `any` stopped on a landing or on a fall onto spikes or past the map.
    let at = centre(harness, player);
    landed
        && (at.y + player_half().y - to.row * TILE).abs() < 1.0
        && grounded(harness, player)
        && at.x >= to.left * TILE + player_half().x
        && at.x <= to.right * TILE - player_half().x
}

/// Launch points on `from`: starts every half tile, nearest the target's
/// midpoint first; for each, three goals; for each, a jump before none, or
/// with `air` the double jump alone. The first that lands.
fn search_launch(
    harness: &mut UiHarness,
    level: &Source,
    from: &Platform,
    to: &Platform,
    air: bool,
) -> Option<(f32, f32, bool)> {
    let midpoint = f32::midpoint(to.left, to.right);
    let goals = [to.left + 0.75, to.right - 0.75, midpoint];
    let mut starts: Vec<f32> = (0..((from.right - from.left) * 2.0) as u32)
        .map(|i| from.left + 0.35 + i as f32 * 0.5)
        .collect();
    starts.sort_by(|a, b| (a - midpoint).abs().total_cmp(&(b - midpoint).abs()));
    for start in starts {
        for goal in goals {
            for jump in [true, false] {
                if (jump || !air)
                    && launch_lands(harness, level, from, to, (start, goal, jump), air)
                {
                    return Some((start, goal, jump));
                }
            }
        }
    }
    None
}

#[test]
fn authored_routes_and_recovery_shelves_traverse_with_real_physics() {
    let level = source();
    let mut harness = bare_level();
    let platforms: BTreeMap<&str, &Platform> = level
        .platforms
        .iter()
        .map(|p| (p.name.as_str(), p))
        .collect();
    let pairs: BTreeSet<(&str, &str)> = level
        .routes
        .values()
        .flat_map(|names| {
            names
                .windows(2)
                .map(|pair| (pair[0].as_str(), pair[1].as_str()))
        })
        .collect();
    let mut failures = Vec::new();
    for &(from, to, ..) in LAUNCHES {
        if !pairs.contains(&(from, to)) {
            failures.push(format!("{from} -> {to}: a LAUNCHES row no route takes"));
        }
    }
    for (route, names) in &level.routes {
        for pair in names.windows(2) {
            let (from, to) = (platforms[pair[0].as_str()], platforms[pair[1].as_str()]);
            let row = LAUNCHES
                .iter()
                .find(|row| row.0 == from.name && row.1 == to.name);
            if let Some(&(_, _, start, goal, jump)) = row
                && launch_lands(&mut harness, &level, from, to, (start, goal, jump), false)
            {
                println!(
                    "{route}: {} -> {} landed on the controller",
                    from.name, to.name
                );
                continue;
            }
            let fix = if row.is_some() {
                "replace its LAUNCHES row with"
            } else {
                "add a LAUNCHES row"
            };
            failures.push(match search_launch(&mut harness, &level, from, to, false) {
                Some((start, goal, jump)) => format!(
                    "{route}: {} -> {}: {fix}: ({:?}, {:?}, {start:?}, {goal:?}, {jump}),",
                    from.name, to.name, from.name, to.name
                ),
                None => format!("{route}: {} -> {} is not traversable", from.name, to.name),
            });
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn each_tomes_support_is_reached_from_its_platform_by_the_double_jump() {
    let level = source();
    let mut harness = bare_level();
    let mut failures = Vec::new();
    for &(name, ..) in TOME_LAUNCHES {
        if !level.tomes.iter().any(|tome| tome.name == name) {
            failures.push(format!("{name}: a TOME_LAUNCHES row no book takes"));
        }
    }
    for tome in &level.tomes {
        let (from, to) = (level.platform(&tome.from), level.platform(&tome.support));
        let row = TOME_LAUNCHES.iter().find(|row| row.0 == tome.name);
        if let Some(&(_, start, goal)) = row
            && launch_lands(&mut harness, &level, from, to, (start, goal, true), true)
        {
            println!(
                "{}: {} -> {} landed by the double jump",
                tome.name, from.name, to.name
            );
            continue;
        }
        let fix = if row.is_some() {
            "replace its TOME_LAUNCHES row with"
        } else {
            "add a TOME_LAUNCHES row"
        };
        failures.push(match search_launch(&mut harness, &level, from, to, true) {
            Some((start, goal, _)) => format!(
                "{}: {fix}: ({:?}, {start:?}, {goal:?}),",
                tome.name, tome.name
            ),
            None => format!(
                "{}: {} -> {} is not reached by the double jump",
                tome.name, from.name, to.name
            ),
        });
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn platforms_support_riders_through_a_complete_cycle_with_real_physics() {
    let mut harness = harness();
    start(&mut harness);
    harness.harness_mut().set_dt(DT);
    let platform = harness
        .world()
        .query::<(Entity, &PathFollower)>()
        .map(|(entity, _)| entity)
        .min_by_key(|entity| entity.id())
        .expect("a moving platform");
    let deck = |harness: &UiHarness| {
        let world = harness.world();
        let at = world.get::<tungsten::core::Position>(platform).unwrap().0;
        let collider = world.get::<Collider>(platform).unwrap();
        let Shape::Aabb { half_extents } = collider.shape else {
            panic!("a deck is a box");
        };
        (at + collider.offset, half_extents)
    };
    let (top, half) = deck(&harness);
    let player = fresh_player(
        &mut harness,
        top - Vec2::new(0.0, half.y + player_half().y + 0.5),
    );
    harness.step(4);
    assert!(grounded(&harness, player), "it stands on the deck");
    // Platform 0's sine takes 5.5 s a cycle: 400 steps cover one whole.
    for frame in 0..400 {
        harness.step(1);
        let (deck_centre, half) = deck(&harness);
        let feet = centre(&harness, player).y + player_half().y;
        let surface = deck_centre.y - half.y;
        assert!(
            (feet - surface).abs() < 1.0,
            "frame {frame}: rider separated from deck: {feet} vs {surface}"
        );
    }
}

#[test]
fn every_platform_sweep_clears_terrain_and_player_headroom() {
    let level = source();
    let mut harness = harness();
    start(&mut harness);
    let world = harness.world();
    let map = world
        .resource::<TilemapRegistry>()
        .get(MAP)
        .expect("the level's map");
    let collision = map
        .layers
        .iter()
        .find(|layer| layer.kind == LayerKind::Collision)
        .expect("a collision layer");
    for placement in level
        .moving_platforms
        .iter()
        .filter(|m| m.elevator.is_none())
    {
        let centre = Vec2::from(placement.position) * TILE;
        let travel = Vec2::from(placement.travel) * TILE;
        // The path's two points and the sine-eased leg between them.
        for step in 0..=200 {
            let eased = 0.5 - 0.5 * (step as f32 / 200.0 * std::f32::consts::PI).cos();
            let pos = centre - travel + 2.0 * travel * eased;
            let half = placement.half_width * TILE;
            let min = pos - Vec2::new(half, DECK_DEPTH * 0.5 + player_half().y * 2.0);
            let max = pos + Vec2::new(half, DECK_DEPTH * 0.5);
            for row in (min.y / TILE).floor() as u32..(max.y / TILE).ceil() as u32 {
                for col in (min.x / TILE).floor() as u32..(max.x / TILE).ceil() as u32 {
                    assert!(
                        collision.tiles[(row * map.width + col) as usize] < 0,
                        "platform at {pos:?} intersects tile {col},{row}"
                    );
                }
            }
        }
    }
}

/// Every static body's box but the lava's surfaces, as min and max.
fn static_boxes(world: &tungsten::core::World) -> Vec<(Vec2, Vec2)> {
    world
        .query::<(&RigidBody, &Collider, &Position)>()
        .filter(|(body, ..)| body.kind == BodyKind::Static && body.layers & LAVA_LAYER == 0)
        .filter_map(|(_, collider, at)| {
            let Shape::Aabb { half_extents } = collider.shape else {
                return None;
            };
            let centre = at.0 + collider.offset;
            Some((centre - half_extents, centre + half_extents))
        })
        .collect()
}

#[test]
fn the_elevator_sweep_clears_tiles_slabs_iron_and_a_riders_headroom() {
    let level = source();
    let lift = level
        .moving_platforms
        .iter()
        .find(|m| m.elevator.is_some())
        .expect("the mine's elevator");
    let rise = lift.elevator.as_ref().unwrap().rise * TILE;
    let mut harness = harness();
    start(&mut harness);
    let world = harness.world();
    let map = world.resource::<TilemapRegistry>().get(MAP).unwrap();
    let collision = map
        .layers
        .iter()
        .find(|layer| layer.kind == LayerKind::Collision)
        .unwrap();
    let boxes = static_boxes(world);
    let half = lift.half_width * TILE;
    let x = lift.position[0] * TILE;
    let rest = lift.position[1] * TILE;
    for step in 0..=400 {
        let top = rest - rise * step as f32 / 400.0;
        // The cage's floor and a rider's head room over it.
        let min = Vec2::new(x - half, top - player_half().y * 2.0);
        let max = Vec2::new(x + half, top + DECK_DEPTH);
        for row in (min.y / TILE).floor() as u32..(max.y / TILE).ceil() as u32 {
            for col in (min.x / TILE).floor() as u32..(max.x / TILE).ceil() as u32 {
                assert!(
                    collision.tiles[(row * map.width + col) as usize] < 0,
                    "the elevator at {top} meets tile {col},{row}"
                );
            }
        }
        for (low, high) in &boxes {
            let inside =
                low.x < max.x - 0.01 && high.x > min.x + 0.01 && low.y < max.y && high.y > min.y;
            assert!(
                !inside,
                "the elevator at {top} meets a static box {low}..{high}"
            );
        }
    }
}

/// The farthest across, in px, the player can still be at or above `dy` px
/// over its take-off, if anywhere: its run times the latest time a jump
/// stays that high, over every air jump's time and a coyote take-off.
fn reach(dy: f32) -> Option<f32> {
    const JUMP: f32 = 1280.0;
    const GRAVITY: f32 = 3600.0;
    const RUN: f32 = 560.0;
    const COYOTE: f32 = 0.1;
    let mut best: Option<f32> = None;
    for coyote in [0.0, COYOTE] {
        // Off an edge for `coyote` seconds first, falling.
        let need = dy + 0.5 * GRAVITY * coyote * coyote;
        for millis in 0..=2000 {
            let a = millis as f32 / 1000.0;
            let at_air_jump = JUMP * a - 0.5 * GRAVITY * a * a;
            let disc = JUMP * JUMP - 2.0 * GRAVITY * (need - at_air_jump);
            if disc >= 0.0 {
                let t = coyote + a + (JUMP + disc.sqrt()) / GRAVITY;
                best = Some(best.map_or(RUN * t, |b: f32| b.max(RUN * t)));
            }
        }
    }
    best
}

#[test]
fn the_mines_upper_level_is_reached_only_by_its_elevator() {
    let level = source();
    let upper = level.platform("mine_upper").clone();
    let (left, right, top) = (upper.left * TILE, upper.right * TILE, upper.row * TILE);
    let mut harness = harness();
    start(&mut harness);
    calm(&mut harness);
    harness.harness_mut().set_dt(DT);
    // Out of every jump: no tile top or static box top in the mine's reach.
    let world = harness.world();
    let map = world.resource::<TilemapRegistry>().get(MAP).unwrap();
    let collision = map
        .layers
        .iter()
        .find(|layer| layer.kind == LayerKind::Collision)
        .unwrap();
    let solid = |col: u32, row: u32| collision.tiles[(row * map.width + col) as usize] >= 0;
    let mut surfaces: Vec<(f32, f32, f32)> = Vec::new();
    for col in 270..map.width.min(380) {
        for row in 1..map.height {
            if solid(col, row) && !solid(col, row - 1) {
                surfaces.push((
                    col as f32 * TILE,
                    (col + 1) as f32 * TILE,
                    row as f32 * TILE,
                ));
            }
        }
    }
    for (low, high) in static_boxes(world) {
        let own = low.y.round() == top && low.x >= left - 0.5 && high.x <= right + 0.5;
        if !own && high.x > 270.0 * TILE && low.x < 380.0 * TILE {
            surfaces.push((low.x, high.x, low.y));
        }
    }
    let body = player_half().x * 2.0;
    let mut nearest = f32::MAX;
    for (x0, x1, y) in surfaces {
        let dy = y - top;
        let gap = (left - x1).max(x0 - right).max(0.0) - body;
        if let Some(across) = reach(dy) {
            nearest = nearest.min(gap - across);
            assert!(
                across < gap,
                "the surface {x0}..{x1} at {y} reaches the upper level: {across} across a gap of {gap}"
            );
        }
    }
    println!("the nearest surface falls {nearest} px short");
    // The elevator carries the player up.
    let world = harness.world();
    let (deck, lift) = world
        .query::<(Entity, &Elevator)>()
        .map(|(entity, lift)| (entity, *lift))
        .next()
        .expect("the elevator");
    assert_eq!(lift.stop, Stop::Bottom);
    let deck_top = |harness: &UiHarness| {
        let world = harness.world();
        let at = world.get::<Position>(deck).unwrap().0;
        let collider = world.get::<Collider>(deck).unwrap();
        let Shape::Aabb { half_extents } = collider.shape else {
            unreachable!()
        };
        at.y + collider.offset.y - half_extents.y
    };
    let stop = |harness: &UiHarness| harness.world().get::<Elevator>(deck).unwrap().stop;
    let middle = harness.world().get::<Position>(deck).unwrap().0.x + 96.0;
    let feet = deck_top(&harness);
    let player = fresh_player(
        &mut harness,
        Vec2::new(middle, feet - player_half().y - 0.5),
    );
    for _ in 0..600 {
        harness.step(1);
        if stop(&harness) == Stop::Top {
            break;
        }
    }
    assert_eq!(stop(&harness), Stop::Top, "carried to the top");
    assert!(
        (deck_top(&harness) - top).abs() < 1.0,
        "flush with the upper level"
    );
    harness.harness_mut().press_action("move_right");
    harness.step(50);
    harness.harness_mut().release_action("move_right");
    harness.step(30);
    let at = centre(&harness, player);
    assert!(
        at.x > left + player_half().x && grounded(&harness, player),
        "on the upper level: {at}"
    );
    assert!(
        (at.y + player_half().y - top).abs() < 2.0,
        "standing on its deck: {at}"
    );
    // Left behind, it goes back down after four seconds.
    harness.step(250);
    for _ in 0..600 {
        if stop(&harness) == Stop::Bottom {
            break;
        }
        harness.step(1);
    }
    assert_eq!(stop(&harness), Stop::Bottom, "back at the bench");
    // Ridden up again, a rider running for the right edge just under the
    // upper level holds it before any overlap, and it goes on once the
    // rider is off.
    let feet = deck_top(&harness);
    let player = fresh_player(
        &mut harness,
        Vec2::new(middle, feet - player_half().y - 0.5),
    );
    let mut held = false;
    let mut running = false;
    for _ in 0..900 {
        harness.step(1);
        if !running && stop(&harness) == Stop::Rising && deck_top(&harness) < top + 120.0 {
            harness.harness_mut().press_action("move_right");
            running = true;
        }
        held |= harness.world().get::<Elevator>(deck).unwrap().held;
        let at = centre(&harness, player);
        let (low, high) = (at - player_half(), at + player_half());
        let inside =
            low.x < right && high.x > left && low.y < top + DECK_DEPTH - 0.5 && high.y > top + 0.5;
        assert!(
            !inside,
            "the rider at {at} is inside the upper level's deck"
        );
        if stop(&harness) == Stop::Top {
            break;
        }
    }
    harness.harness_mut().release_action("move_right");
    assert!(running && held, "the overhanging rider held it");
    assert_eq!(stop(&harness), Stop::Top, "it went on to the top");
}

/// The run's elevator, its deck and its state.
fn elevator(harness: &UiHarness) -> (Entity, Elevator) {
    harness
        .world()
        .query::<(Entity, &Elevator)>()
        .map(|(entity, lift)| (entity, *lift))
        .next()
        .expect("the elevator")
}

/// The deck's box: its min and max corners.
fn deck_box(harness: &UiHarness, deck: Entity) -> (Vec2, Vec2) {
    let world = harness.world();
    let at = world.get::<Position>(deck).unwrap().0;
    let collider = world.get::<Collider>(deck).unwrap();
    let Shape::Aabb { half_extents } = collider.shape else {
        unreachable!()
    };
    let centre = at + collider.offset;
    (centre - half_extents, centre + half_extents)
}

/// Whether the player's body is inside the deck by more than half a pixel.
fn inside_deck(harness: &UiHarness, deck: Entity, player: Entity) -> bool {
    let (min, max) = deck_box(harness, deck);
    let at = centre(harness, player);
    let (low, high) = (at - player_half(), at + player_half());
    high.x.min(max.x) - low.x.max(min.x) > 0.5 && high.y.min(max.y) - low.y.max(min.y) > 0.5
}

/// The run on the map at a fixed step a frame, its enemies off, with the
/// elevator at its top stop, left there unridden.
fn lift_at_top() -> (UiHarness, Entity, Elevator) {
    let mut harness = harness();
    start(&mut harness);
    calm(&mut harness);
    harness.harness_mut().set_dt(DT);
    let (deck, lift) = elevator(&harness);
    let world = harness.world_mut();
    world.get_mut::<Position>(deck).unwrap().0 = lift.top;
    world.get_mut::<PrevPosition>(deck).unwrap().0 = lift.top;
    world.get_mut::<Elevator>(deck).unwrap().stop = Stop::Top;
    harness.step(1);
    (harness, deck, lift)
}

/// Steps until the elevator, left at its top, has come down on `player`,
/// turned back and stood at its top again, at most 900 frames; the stops it
/// passed through, the player never inside its deck.
fn turned_back(harness: &mut UiHarness, deck: Entity, player: Entity) -> Vec<Stop> {
    let mut stops = vec![Stop::Top];
    for frame in 0..900 {
        harness.step(1);
        assert!(
            !inside_deck(harness, deck, player),
            "frame {frame}: the deck {:?} over the player at {}",
            deck_box(harness, deck),
            centre(harness, player)
        );
        let stop = elevator(harness).1.stop;
        if stops.last() != Some(&stop) {
            stops.push(stop);
        }
        if stops.len() == 4 {
            break;
        }
    }
    stops
}

#[test]
fn the_elevator_turns_back_over_a_player_and_sets_one_inside_out() {
    let level = source();
    let bed = level.platform("shaft_bed").row * TILE;
    // A player standing on the shaft's bed under the bottom stop.
    let (mut harness, deck, _) = lift_at_top();
    let middle = deck_box(&harness, deck).0.x + 96.0;
    let player = fresh_player(&mut harness, Vec2::new(middle, bed - player_half().y - 0.5));
    let stops = turned_back(&mut harness, deck, player);
    assert_eq!(
        stops,
        [Stop::Top, Stop::Falling, Stop::Rising, Stop::Top],
        "it turned back over the player on the bed"
    );
    // A player standing on a slab halfway down, which the deck meets at its
    // eased path's top speed.
    let (mut harness, deck, lift) = lift_at_top();
    let half_way = (lift.top.y + lift.bottom.y) * 0.5;
    let (min, max) = deck_box(&harness, deck);
    let head = half_way + (max.y - min.y) + (min.y - lift.top.y) + 4.0;
    let feet = head + player_half().y * 2.0;
    super::floor(
        &mut harness,
        Vec2::new(middle, feet + 10.0),
        Vec2::new(96.0, 10.0),
    );
    harness.step(1);
    let player = fresh_player(
        &mut harness,
        Vec2::new(middle, feet - player_half().y - 0.5),
    );
    let stops = turned_back(&mut harness, deck, player);
    assert_eq!(
        stops,
        [Stop::Top, Stop::Falling, Stop::Rising, Stop::Top],
        "it turned back over the player on the slab"
    );
    // At rest, a player inside its deck's top and one inside its bottom:
    // each set out, the first standing on it.
    for (inset, stands) in [(6.0, true), (-6.0, false)] {
        let mut rest = super::harness();
        start(&mut rest);
        calm(&mut rest);
        rest.harness_mut().set_dt(DT);
        let (deck, lift) = elevator(&rest);
        assert_eq!(lift.stop, Stop::Bottom);
        let (min, max) = deck_box(&rest, deck);
        let y = if inset > 0.0 {
            min.y + inset
        } else {
            max.y + inset
        };
        let player = fresh_player(&mut rest, Vec2::new(middle, y));
        rest.step(2);
        assert!(!inside_deck(&rest, deck, player), "set out from {y}");
        if stands {
            let ground = rest
                .world()
                .get::<PlatformerController>(player)
                .unwrap()
                .ground();
            assert_eq!(ground, Some(deck), "standing on it");
        }
    }
}

#[test]
fn a_still_rider_idles_on_the_rising_elevator() {
    let mut harness = harness();
    start(&mut harness);
    calm(&mut harness);
    harness.harness_mut().set_dt(DT);
    let (deck, _) = elevator(&harness);
    let (min, _) = deck_box(&harness, deck);
    let middle = min.x + 96.0;
    let player = fresh_player(
        &mut harness,
        Vec2::new(middle, min.y - player_half().y - 0.5),
    );
    let state = |harness: &UiHarness| {
        harness
            .world()
            .get::<CharacterAnimator>(player)
            .unwrap()
            .state()
    };
    let mut rising = 0;
    for _ in 0..600 {
        harness.step(1);
        match elevator(&harness).1.stop {
            Stop::Rising => {
                rising += 1;
                assert_eq!(state(&harness), "idle", "a still rider, {rising} frames up");
            }
            Stop::Top => break,
            _ => {}
        }
    }
    assert!(rising > 60, "it rose with the rider: {rising} frames");
}

#[test]
fn thin_platform_underside_and_edges_match_visible_art() {
    let level = source();
    let deck = level.platform(UNDERSIDE_DECK);
    let mut harness = bare_level();
    harness.world_mut().resource_mut::<PhysicsConfig>().gravity = Vec2::ZERO;
    // Below the transparent portion of the deck's tiles, horizontal movement
    // stays unobstructed: no full 64 px tile collides there.
    let below = Vec2::new((deck.left + 1.5) * TILE, deck.row * TILE + 56.0);
    let player = fresh_player(&mut harness, below);
    harness.step(1);
    harness.harness_mut().press_action("move_right");
    harness.step(45);
    harness.harness_mut().release_action("move_right");
    let at = centre(&harness, player);
    assert!(
        at.x > (deck.left + 3.5) * TILE,
        "it passed under the deck: {at}"
    );
    assert!((at.y - below.y).abs() < 0.5, "and kept its height: {at}");
    // A rising player meets the visible underside at row * 64 + 23, not + 64.
    let player = fresh_player(
        &mut harness,
        Vec2::new((deck.left + 5.0) * TILE, (deck.row + 1.0) * TILE + 30.0),
    );
    harness.step(1);
    let controller = harness
        .world_mut()
        .get_mut::<PlatformerController>(player)
        .unwrap();
    controller.launch(Vec2::new(0.0, -240.0));
    harness.step(40);
    let head = centre(&harness, player).y - player_half().y;
    assert!(
        (head - (deck.row * TILE + DECK_DEPTH)).abs() < 1.0,
        "head stopped at {head}"
    );
}

#[test]
fn each_slab_platform_and_iron_box_spawns_one_collider_with_no_join_inside() {
    let mut harness = harness();
    start(&mut harness);
    let world = harness.world();
    let map = world
        .resource::<TilemapRegistry>()
        .get(MAP)
        .expect("the level's map");
    let slabs: Vec<[f32; 4]> = map
        .object_layers
        .iter()
        .filter(|layer| layer.name == "slabs" || layer.name == "iron")
        .flat_map(|layer| &layer.objects)
        .map(|object| {
            [
                object.position.x,
                object.position.y,
                object.size.x,
                object.size.y,
            ]
        })
        .collect();
    assert!(!slabs.is_empty());
    for a in &slabs {
        for b in &slabs {
            assert!(
                !(a[1] == b[1] && a[3] == b[3] && a[0] + a[2] == b[0]),
                "{a:?} and {b:?} still join"
            );
        }
    }
    // Every static body is a slab's or an iron box's, its box the object's,
    // the start's boundary (01 (k)), or a lava basin's surface, in its own
    // layer.
    let surfaces = world
        .query::<&RigidBody>()
        .filter(|body| body.kind == BodyKind::Static && body.layers & LAVA_LAYER != 0)
        .count();
    assert_eq!(surfaces, 3, "a surface over each pool");
    let mut boxes: Vec<[f32; 4]> = world
        .query::<(
            &RigidBody,
            &Collider,
            &tungsten::core::Position,
            Option<&Sprite>,
        )>()
        .filter(|(body, ..)| body.kind == BodyKind::Static && body.layers & LAVA_LAYER == 0)
        .map(|(_, collider, at, sprite)| {
            assert!(sprite.is_none(), "a slab draws nothing: its tiles do");
            let Shape::Aabb { half_extents } = collider.shape else {
                panic!("a slab or an iron box is a box");
            };
            let min = at.0 + collider.offset - half_extents;
            [min.x, min.y, 2.0 * half_extents.x, 2.0 * half_extents.y]
        })
        .collect();
    let mut want = slabs.clone();
    let [min, max] = BOUNDARY;
    want.push([min.x, min.y, max.x - min.x, max.y - min.y]);
    let key = |b: &[f32; 4]| (b[1] as i64, b[0] as i64, b[2] as i64, b[3] as i64);
    boxes.sort_by_key(key);
    want.sort_by_key(key);
    assert_eq!(boxes, want, "one collider per slab and iron object");
}

#[test]
fn the_start_wall_and_its_boundary_keep_a_thrown_ball_in_the_map() {
    // A bowling ball flung from over the clearing crosses x 0 near row 18,
    // over the wall's top at row 21: the boundary turns it back, where
    // without it `toys::bounds` would take it past x -128.
    let mut harness = harness();
    start(&mut harness);
    harness.harness_mut().set_dt(DT);
    let thrown = ball(&mut harness, Vec2::new(6.0, 18.0) * TILE, false);
    harness
        .world_mut()
        .get_mut::<Velocity>(thrown)
        .expect("a ball's velocity")
        .0 = Vec2::new(-1200.0, -600.0);
    harness.step(180);
    let world = harness.world();
    assert!(world.is_alive(thrown), "the ball left the map");
    let at = world.get::<Position>(thrown).expect("a ball's point").0;
    assert!(at.x >= 0.0, "the ball at {at}");
}
