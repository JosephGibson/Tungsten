use super::*;

fn route_jump_possible(
    from: &crate::level_layout::RoutePlatform,
    to: &crate::level_layout::RoutePlatform,
    dt: f32,
) -> bool {
    // Search launch points on the authored platform, using ordinary input and
    // real map collision. No teleport, impulse tweak or collider substitution
    // occurs during any tested traversal segment.
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
                let mut harness = platformer_harness(&PHYSICS_SYSTEMS);
                seed_level(harness.world_mut());
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
                    continue;
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
                            let delta =
                                (position - Vec2::from_array(h.motion.position) * TILE).abs();
                            delta.cmplt(PLAYER_HALF + Vec2::new(27.0, 14.0)).all()
                        })
                    {
                        break;
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
                        break;
                    }
                }
            }
        }
    }
    false
}

#[test]
fn authored_routes_and_recovery_shelves_traverse_with_real_physics() {
    use crate::level_layout::{PLATFORMS, ROUTES};
    let mut tested = std::collections::HashSet::new();
    for &(route, names) in ROUTES {
        for pair in names.windows(2) {
            if !tested.insert((pair[0], pair[1])) {
                continue;
            }
            let from = PLATFORMS.iter().find(|p| p.name == pair[0]).unwrap();
            let to = PLATFORMS.iter().find(|p| p.name == pair[1]).unwrap();
            assert!(
                route_jump_possible(from, to, 1.0 / 60.0),
                "{route}: {} -> {} is not traversable",
                from.name,
                to.name
            );
        }
    }
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
