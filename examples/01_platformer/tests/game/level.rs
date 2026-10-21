//! The level from its generator, on the kit's controller and path follower:
//! every route's jumps, a ride through a platform's cycle, the platforms'
//! sweeps, a thin deck's underside, and one collider per slab.

use std::collections::{BTreeMap, BTreeSet};

use example_01_platformer::features::level::components::Hazard;
use example_01_platformer::features::level::{MAP, TILE};
use example_01_platformer::features::player::{self, PLAYER_HALF};
use example_01_platformer::features::run::Run;
use example_01_platformer::shell::Difficulty;
use glam::Vec2;
use serde::Deserialize;
use tungsten::core::{
    Collider, Entity, LayerKind, PhysicsConfig, RigidBody, Shape, Sprite, TilemapRegistry,
};
use tungsten::kit::{PathFollower, PlatformerController};
use tungsten::physics::BodyKind;
use tungsten::testing::UiHarness;

use super::{harness, start};

const DT: f32 = 1.0 / 60.0;

/// `tools/level.json`, the generator's source, as far as these tests read it.
#[derive(Deserialize)]
struct Source {
    platforms: Vec<Platform>,
    routes: BTreeMap<String, Vec<String>>,
    hazards: Vec<SourceHazard>,
    moving_platforms: Vec<Moving>,
    kill_row: f32,
}

#[derive(Debug, Clone, Deserialize)]
struct Platform {
    name: String,
    left: f32,
    right: f32,
    row: f32,
}

#[derive(Deserialize)]
struct SourceHazard {
    fire: bool,
    position: [f32; 2],
}

#[derive(Deserialize)]
struct Moving {
    position: [f32; 2],
    travel: [f32; 2],
    half_width: f32,
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
    ("terrace_a", "forest_perch", 16.35, 12.75, true),
    ("forest_perch", "forest_shrine", 12.35, 8.75, true),
    ("climb_b", "grotto", 67.85, 74.25, true),
    ("grotto", "grotto_step", 71.85, 73.75, true),
    ("grotto_step", "grotto_exit", 75.85, 77.75, true),
    ("grotto_exit", "grotto_return", 78.85, 80.75, true),
    ("grotto_return", "climb_d", 80.85, 74.75, true),
    ("grotto", "grotto_stone", 70.35, 70.75, true),
    ("grotto_stone", "grotto_step", 71.85, 73.75, true),
    ("clearing", "terrace_a", 15.85, 16.75, true),
    ("terrace_a", "terrace_b", 18.35, 23.25, true),
    ("terrace_b", "lower_a", 23.85, 24.75, true),
    ("lower_a", "lower_b", 31.85, 34.75, true),
    ("lower_b", "aqueduct_step", 37.85, 38.75, true),
    ("aqueduct_step", "lower_b_exit", 40.85, 41.75, true),
    ("lower_b_exit", "lower_c", 43.85, 46.75, true),
    ("lower_c", "climb_a", 55.85, 56.75, true),
    ("climb_a", "climb_b", 60.85, 62.75, true),
    ("climb_b", "climb_c", 66.85, 68.75, true),
    ("climb_c", "climb_d", 72.85, 74.75, true),
    ("climb_d", "climb_e", 78.85, 80.75, true),
    ("climb_e", "climb_f", 84.85, 86.75, true),
    ("climb_f", "tower_lower_a", 91.85, 92.75, true),
    ("tower_lower_a", "tower_lower_b", 96.85, 98.75, true),
    ("tower_lower_b", "tower_lower_c", 102.85, 104.75, true),
    ("tower_lower_c", "gate", 107.85, 108.75, true),
    ("gate", "overlook_a", 114.35, 116.75, true),
    ("overlook_a", "overlook_b", 119.85, 123.75, true),
    ("recovery_a", "lower_b", 33.85, 34.75, true),
    ("recovery_b", "lower_c", 45.85, 46.75, true),
    ("gate", "gate_step", 118.85, 119.75, true),
    ("gate_step", "gate_crest", 120.85, 121.75, true),
    ("gate_crest", "overlook_a", 121.35, 116.75, true),
    ("terrace_b", "upper_a", 22.85, 24.75, true),
    ("upper_a", "upper_b", 29.85, 32.75, true),
    ("upper_b", "upper_c", 37.85, 40.75, true),
    ("upper_c", "upper_d", 45.85, 48.75, true),
    ("upper_d", "climb_a", 53.85, 56.75, true),
    ("climb_f", "switch_a", 86.85, 80.75, true),
    ("switch_a", "switch_b", 80.85, 74.75, true),
    ("switch_b", "switch_c", 78.85, 80.75, true),
    ("switch_c", "tower_upper_a", 85.85, 88.75, true),
    ("tower_upper_a", "tower_upper_b", 95.85, 98.75, true),
    ("tower_upper_b", "gate", 105.85, 108.75, true),
    ("wet_shelf", "grotto", 75.85, 66.75, true),
];

/// A run with no hazard or moving platform, so a route's jumps meet the map
/// alone, at a fixed step a frame.
fn bare_level() -> UiHarness {
    let mut harness = harness();
    start(&mut harness);
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
    let player = player::spawn(world, centre, Difficulty::Normal);
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

/// One attempt with ordinary input on the real map: the player settles at
/// `start` on `from`, steers toward `goal`, holding jump through the rise
/// when it jumps so the jump cut never acts, and must stand on `to`.
fn launch_lands(
    harness: &mut UiHarness,
    level: &Source,
    from: &Platform,
    to: &Platform,
    (start, goal, jump): (f32, f32, bool),
) -> bool {
    // A stale row's start may lie off `from`; one on `to` would land at once.
    if !(from.left..from.right).contains(&start) {
        return false;
    }
    release_all(harness);
    let at = Vec2::new(start * TILE, from.row * TILE - PLAYER_HALF.y - 0.5);
    let player = fresh_player(harness, at);
    harness.step(4);
    let settled = centre(harness, player);
    if !grounded(harness, player) || (settled.y + PLAYER_HALF.y - from.row * TILE).abs() > 1.0 {
        return false;
    }
    let spikes: Vec<Vec2> = level
        .hazards
        .iter()
        .filter(|hazard| !hazard.fire)
        .map(|hazard| Vec2::from(hazard.position) * TILE)
        .collect();
    let landed = (0..(2.5 / DT) as usize).any(|tick| {
        let dx = goal * TILE - centre(harness, player).x;
        let input = harness.harness_mut();
        input.release_action("move_left");
        input.release_action("move_right");
        if dx.abs() > 560.0 * DT * 0.6 {
            input.press_action(if dx > 0.0 { "move_right" } else { "move_left" });
        }
        if tick == 0 && jump {
            input.press_action("jump");
        }
        harness.step(1);
        let at = centre(harness, player);
        let spiked = spikes.iter().any(|spike| {
            let reach = PLAYER_HALF + SPIKES_HALF;
            (at - *spike).abs().cmplt(reach).all()
        });
        if spiked || at.y > level.kill_row * TILE {
            return true;
        }
        tick > 3
            && grounded(harness, player)
            && (at.y + PLAYER_HALF.y - to.row * TILE).abs() < 1.0
            && at.x >= to.left * TILE + PLAYER_HALF.x
            && at.x <= to.right * TILE - PLAYER_HALF.x
    });
    release_all(harness);
    // `any` stopped on a landing or on a fall onto spikes or past the map.
    let at = centre(harness, player);
    landed
        && (at.y + PLAYER_HALF.y - to.row * TILE).abs() < 1.0
        && grounded(harness, player)
        && at.x >= to.left * TILE + PLAYER_HALF.x
        && at.x <= to.right * TILE - PLAYER_HALF.x
}

/// Launch points on `from`: starts every half tile, nearest the target's
/// midpoint first; for each, three goals; for each, a jump before none.
/// The first that lands.
fn search_launch(
    harness: &mut UiHarness,
    level: &Source,
    from: &Platform,
    to: &Platform,
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
                if launch_lands(harness, level, from, to, (start, goal, jump)) {
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
                && launch_lands(&mut harness, &level, from, to, (start, goal, jump))
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
            failures.push(match search_launch(&mut harness, &level, from, to) {
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
        top - Vec2::new(0.0, half.y + PLAYER_HALF.y + 0.5),
    );
    harness.step(4);
    assert!(grounded(&harness, player), "it stands on the deck");
    // Platform 0's sine takes 5.5 s a cycle: 400 steps cover one whole.
    for frame in 0..400 {
        harness.step(1);
        let (deck_centre, half) = deck(&harness);
        let feet = centre(&harness, player).y + PLAYER_HALF.y;
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
    for placement in &level.moving_platforms {
        let centre = Vec2::from(placement.position) * TILE;
        let travel = Vec2::from(placement.travel) * TILE;
        // The path's two points and the sine-eased leg between them.
        for step in 0..=200 {
            let eased = 0.5 - 0.5 * (step as f32 / 200.0 * std::f32::consts::PI).cos();
            let pos = centre - travel + 2.0 * travel * eased;
            let half = placement.half_width * TILE;
            let min = pos - Vec2::new(half, DECK_DEPTH * 0.5 + PLAYER_HALF.y * 2.0);
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

#[test]
fn thin_platform_underside_and_edges_match_visible_art() {
    let mut harness = bare_level();
    harness.world_mut().resource_mut::<PhysicsConfig>().gravity = Vec2::ZERO;
    // Below the transparent portion of the bridge tile, horizontal movement
    // stays unobstructed: no full 64 px tile collides there.
    let below = Vec2::new(23.5 * TILE, 30.0 * TILE + 56.0);
    let player = fresh_player(&mut harness, below);
    harness.step(1);
    harness.harness_mut().press_action("move_right");
    harness.step(45);
    harness.harness_mut().release_action("move_right");
    let at = centre(&harness, player);
    assert!(at.x > 25.5 * TILE, "it passed under the bridge: {at}");
    assert!((at.y - below.y).abs() < 0.5, "and kept its height: {at}");
    // A rising player meets the visible underside at row * 64 + 23, not + 64.
    let player = fresh_player(&mut harness, Vec2::new(27.0 * TILE, 31.0 * TILE + 30.0));
    harness.step(1);
    let controller = harness
        .world_mut()
        .get_mut::<PlatformerController>(player)
        .unwrap();
    controller.launch(Vec2::new(0.0, -240.0));
    harness.step(40);
    let head = centre(&harness, player).y - PLAYER_HALF.y;
    assert!(
        (head - (30.0 * TILE + DECK_DEPTH)).abs() < 1.0,
        "head stopped at {head}"
    );
}

#[test]
fn each_slab_platform_spawns_one_collider_with_no_join_inside() {
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
        .filter(|layer| layer.name == "slabs")
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
    // Every static body is a slab's, its box the slab object's.
    let mut boxes: Vec<[f32; 4]> = world
        .query::<(
            &RigidBody,
            &Collider,
            &tungsten::core::Position,
            Option<&Sprite>,
        )>()
        .filter(|(body, ..)| body.kind == BodyKind::Static)
        .map(|(_, collider, at, sprite)| {
            assert!(sprite.is_none(), "a slab draws nothing: its tiles do");
            let Shape::Aabb { half_extents } = collider.shape else {
                panic!("a slab is a box");
            };
            let min = at.0 + collider.offset - half_extents;
            [min.x, min.y, 2.0 * half_extents.x, 2.0 * half_extents.y]
        })
        .collect();
    let mut want = slabs.clone();
    let key = |b: &[f32; 4]| (b[1] as i64, b[0] as i64, b[2] as i64, b[3] as i64);
    boxes.sort_by_key(key);
    want.sort_by_key(key);
    assert_eq!(boxes, want, "one collider per slab object");
}
