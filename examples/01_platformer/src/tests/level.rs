use super::*;

use crate::level_layout::RoutePlatform;

/// The launch that lands each authored route segment, as `search_launch` finds
/// it first: the platforms' names, start and goal x in tiles, and whether the
/// player jumps on the first tick. The route test replays these and searches
/// again only for a segment whose row is missing or no longer lands.
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
    ("overlook_a", "overlook_b", 120.85, 123.75, true),
    ("recovery_a", "lower_b", 33.85, 34.75, true),
    ("recovery_b", "lower_c", 45.85, 46.75, true),
    ("gate", "gate_step", 119.35, 119.75, true),
    ("gate_step", "gate_crest", 121.35, 121.75, true),
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

/// One attempt with ordinary input and real map collision; no teleport,
/// impulse tweak or collider substitution occurs. The player settles at
/// `start` on `from`, steers toward `goal` and must stand on `to`.
fn launch_lands(
    map: &TilemapData,
    from: &RoutePlatform,
    to: &RoutePlatform,
    (start, goal, jump): (f32, f32, bool),
    dt: f32,
) -> bool {
    // A stale row's start may lie off `from`; one on `to` would land at once.
    if !(from.left..from.right).contains(&start) {
        return false;
    }
    let mut harness = platformer_harness(&PHYSICS_SYSTEMS);
    seed_level_map(harness.world_mut(), map.clone());
    harness.set_dt(dt);
    let player = spawn_test_player(
        harness.world_mut(),
        Vec2::new(start * TILE, from.row * TILE - PLAYER_HALF.y - 0.5),
    );
    harness.step(4);
    let world = harness.world();
    let initial = world.get::<Position>(player).unwrap().0;
    if !world.get::<Player>(player).unwrap().grounded
        || (initial.y + PLAYER_HALF.y - from.row * TILE).abs() > 1.0
    {
        return false;
    }
    for tick in 0..(2.5 / dt) as usize {
        let position = harness.world().get::<Position>(player).unwrap().0;
        let dx = goal * TILE - position.x;
        let input = input_mut(&mut harness);
        input.key_up(KeyCode::KeyD);
        input.key_up(KeyCode::KeyA);
        input.key_up(KeyCode::Space);
        if dx.abs() > crate::state::PLAYER_MOVE_SPEED * dt * 0.6 {
            input.key_down(if dx > 0.0 {
                KeyCode::KeyD
            } else {
                KeyCode::KeyA
            });
        }
        if tick == 0 && jump {
            input.key_down(KeyCode::Space);
        }
        harness.step(1);
        let world = harness.world();
        let position = world.get::<Position>(player).unwrap().0;
        if crate::level_layout::HAZARDS
            .iter()
            .filter(|h| !h.fire)
            .any(|h| {
                let delta = (position - Vec2::from_array(h.motion.position) * TILE).abs();
                delta.cmplt(PLAYER_HALF + Vec2::new(27.0, 14.0)).all()
            })
        {
            return false;
        }
        if tick > 3
            && world.get::<Player>(player).unwrap().grounded
            && (position.y + PLAYER_HALF.y - to.row * TILE).abs() < 1.0
            && position.x >= to.left * TILE + PLAYER_HALF.x
            && position.x <= to.right * TILE - PLAYER_HALF.x
        {
            return true;
        }
        if position.y > crate::state::KILL_Y {
            return false;
        }
    }
    false
}

/// Searches launch points on the authored platform: starts every half tile,
/// nearest the target's midpoint first; for each, three goals; for each, a jump
/// before none. Returns the first launch that lands.
fn search_launch(
    map: &TilemapData,
    from: &RoutePlatform,
    to: &RoutePlatform,
    dt: f32,
) -> Option<(f32, f32, bool)> {
    let goals = [
        to.left + 0.75,
        to.right - 0.75,
        f32::midpoint(to.left, to.right),
    ];
    let mut starts: Vec<_> = (0..((from.right - from.left) * 2.0) as u32)
        .map(|i| from.left + 0.35 + i as f32 * 0.5)
        .collect();
    starts.sort_by(|a, b| {
        let midpoint = f32::midpoint(to.left, to.right);
        (a - midpoint).abs().total_cmp(&(b - midpoint).abs())
    });
    for start in starts {
        for goal in goals {
            for jump in [true, false] {
                if launch_lands(map, from, to, (start, goal, jump), dt) {
                    return Some((start, goal, jump));
                }
            }
        }
    }
    None
}

#[test]
fn authored_routes_and_recovery_shelves_traverse_with_real_physics() {
    use crate::level_layout::{PLATFORMS, ROUTES};
    let map = level_map();
    let dt = 1.0 / 60.0;
    let mut tested = std::collections::HashSet::new();
    let mut failures = Vec::new();
    for &(route, names) in ROUTES {
        for pair in names.windows(2) {
            if !tested.insert((pair[0], pair[1])) {
                continue;
            }
            let from = PLATFORMS.iter().find(|p| p.name == pair[0]).unwrap();
            let to = PLATFORMS.iter().find(|p| p.name == pair[1]).unwrap();
            let row = LAUNCHES
                .iter()
                .find(|row| row.0 == from.name && row.1 == to.name);
            if row.is_some_and(|&(_, _, start, goal, jump)| {
                launch_lands(&map, from, to, (start, goal, jump), dt)
            }) {
                continue;
            }
            let fix = if row.is_some() {
                "replace its LAUNCHES row with"
            } else {
                "add a LAUNCHES row"
            };
            failures.push(match search_launch(&map, from, to, dt) {
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
fn prescribed_platforms_carry_supported_players_and_release_jumps() {
    use crate::gameplay::*;
    let mut world = seed_world();
    spawn_obstacles(&mut world);
    let (platform, half) = world
        .query::<MovingPlatform>()
        .next()
        .map(|(e, p)| (e, p.half_width))
        .unwrap();
    assert!(half > PLAYER_HALF.x);
    let original = world.get::<Position>(platform).unwrap().0;
    let player = spawn_test_player(
        &mut world,
        original - Vec2::new(0.0, crate::level_layout::DECK_DEPTH * 0.5 + PLAYER_HALF.y),
    );
    move_obstacles(&mut world);
    let delta = world.get::<Position>(platform).unwrap().0 - original;
    assert!(delta.length() > 0.0);
    assert!(
        (world.get::<Position>(player).unwrap().0
            - (original + delta
                - Vec2::new(0.0, crate::level_layout::DECK_DEPTH * 0.5 + PLAYER_HALF.y)))
        .length()
            < 0.01
    );
    let before = world.get::<Position>(player).unwrap().0;
    world.get_mut::<Velocity>(player).unwrap().0.y = -crate::state::PLAYER_JUMP_IMPULSE;
    move_obstacles(&mut world);
    assert_eq!(world.get::<Position>(player).unwrap().0, before);
}

#[test]
fn platforms_support_riders_through_a_complete_cycle_with_real_physics() {
    use crate::gameplay::*;
    let mut harness = platformer_harness(&[
        ("player_input", player_input),
        ("move_obstacles", move_obstacles),
        ("physics_step", physics_step),
        ("ground_detection", ground_detection),
    ]);
    let world = harness.world_mut();
    spawn_obstacles(world);
    let platform = world.query::<MovingPlatform>().next().unwrap().0;
    let start = world.get::<Position>(platform).unwrap().0;
    let player = spawn_test_player(
        world,
        start - Vec2::new(0.0, crate::level_layout::DECK_DEPTH * 0.5 + PLAYER_HALF.y),
    );
    for _ in 0..400 {
        harness.step(1);
        let world = harness.world();
        let feet = world.get::<Position>(player).unwrap().0.y + PLAYER_HALF.y;
        let deck =
            world.get::<Position>(platform).unwrap().0.y - crate::level_layout::DECK_DEPTH * 0.5;
        assert!(
            (feet - deck).abs() < 1.0,
            "rider separated from deck: {feet} vs {deck}"
        );
    }
}

#[test]
fn every_platform_sweep_clears_terrain_and_player_headroom() {
    let world = real_level_world();
    let map = world
        .get_resource::<TilemapRegistry>()
        .unwrap()
        .get("ex10_level")
        .unwrap();
    let collision = map
        .layers
        .iter()
        .find(|l| l.kind == LayerKind::Collision)
        .unwrap();
    for placement in crate::level_layout::MOVING_PLATFORMS {
        for step in 0..200 {
            let m = placement.motion;
            let pos = (Vec2::from_array(m.position)
                + Vec2::from_array(m.travel) * (step as f32 / 200.0 * std::f32::consts::TAU).sin())
                * TILE;
            let min = pos
                - Vec2::new(
                    placement.half_width * TILE,
                    crate::level_layout::DECK_DEPTH * 0.5 + PLAYER_HALF.y * 2.0,
                );
            let max = pos
                + Vec2::new(
                    placement.half_width * TILE,
                    crate::level_layout::DECK_DEPTH * 0.5,
                );
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
    let mut world = real_level_world();
    world.get_resource_mut::<PhysicsConfig>().unwrap().gravity = Vec2::ZERO;
    // Below the transparent portion of the bridge tile, horizontal movement
    // must stay unobstructed. This used to collide with a full 64px tile.
    let player = spawn_test_player(&mut world, Vec2::new(23.5 * TILE, 30.0 * TILE + 56.0));
    world.get_mut::<Velocity>(player).unwrap().0 = Vec2::new(200.0, 0.0);
    for _ in 0..45 {
        physics_step(&mut world);
    }
    assert!(world.get::<Position>(player).unwrap().0.x > 25.5 * TILE);
    assert!((world.get::<Position>(player).unwrap().0.y - (30.0 * TILE + 56.0)).abs() < 0.5);
    // A rising player contacts the visible underside at row*64+23, not +64.
    world.insert(player, Position(Vec2::new(27.0 * TILE, 31.0 * TILE + 30.0)));
    world.insert(player, Velocity(Vec2::new(0.0, -240.0)));
    for _ in 0..40 {
        physics_step(&mut world);
    }
    let head = world.get::<Position>(player).unwrap().0.y - PLAYER_HALF.y;
    assert!(
        (head - (30.0 * TILE + crate::level_layout::DECK_DEPTH)).abs() < 1.0,
        "head stopped at {head}"
    );
}
