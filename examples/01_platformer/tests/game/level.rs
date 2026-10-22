//! The level from its generator, on the kit's controller and path follower:
//! every route's jumps, a ride through a platform's cycle, the platforms'
//! sweeps, a thin deck's underside, and one collider per slab and iron box.

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

use super::{calm, harness, start};

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
    ("hollow_floor_1", "stepping_stone", 231.85, 235.75, true),
    ("stepping_stone", "hollow_floor_2", 236.85, 240.75, true),
    ("hollow_floor_2", "hollow_step", 249.85, 250.75, true),
    ("hollow_step", "hollow_shelf_a", 253.85, 254.75, true),
    ("hollow_shelf_a", "hollow_shelf_b", 259.85, 262.75, true),
    ("hollow_shelf_b", "gallery", 267.85, 268.75, true),
    ("gallery", "cut_floor_1", 282.85, 283.75, true),
    ("cut_floor_1", "cut_floor_2", 305.35, 306.75, true),
    ("cut_floor_2", "cut_floor_3", 323.35, 324.75, true),
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
    let head = centre(&harness, player).y - PLAYER_HALF.y;
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
    // Every static body is a slab's or an iron box's, its box the object's.
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
                panic!("a slab or an iron box is a box");
            };
            let min = at.0 + collider.offset - half_extents;
            [min.x, min.y, 2.0 * half_extents.x, 2.0 * half_extents.y]
        })
        .collect();
    let mut want = slabs.clone();
    let key = |b: &[f32; 4]| (b[1] as i64, b[0] as i64, b[2] as i64, b[3] as i64);
    boxes.sort_by_key(key);
    want.sort_by_key(key);
    assert_eq!(boxes, want, "one collider per slab and iron object");
}
