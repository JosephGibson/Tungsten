use super::*;
use crate::assets::{LayerPhysics, Slope, SlopeCorner, SlopeRise, TileProps, TilemapData};
use crate::physics::RigidBodyBundle;

const WALL: TileIndex = 0;
const LEDGE: TileIndex = 1;
const RAMP: TileIndex = 2;

/// A 20 × 20 map of 16 px cells at the origin, its collision layer `walls`
/// holding `cells`, with tiles `wall`, `ledge` (one-way) and `ramp` (a
/// slope), and the layer's physics `physics`.
fn world_with(cells: &[(u32, u32, TileIndex)], physics: LayerPhysics) -> World {
    let mut map = TilemapData::new(16, 16, 20, 20);
    map.add_tile("wall");
    map.add_tile("ledge");
    map.add_tile("ramp");
    map.set_tile_props(
        LEDGE,
        TileProps {
            one_way: true,
            ..TileProps::default()
        },
    );
    map.set_tile_props(
        RAMP,
        TileProps {
            slope: Some(Slope {
                rise: SlopeRise::Full,
                corner: SlopeCorner::BottomRight,
            }),
            ..TileProps::default()
        },
    );
    let walls = map.add_layer("walls", LayerKind::Collision);
    map.set_layer_physics(walls, physics);
    for &(col, row, tile) in cells {
        map.set_tile(walls, col, row, tile);
    }
    let mut maps = TilemapRegistry::new();
    maps.insert("room".into(), map);
    let mut world = World::new();
    world.insert_resource(maps);
    world.insert_resource(PhysicsConfig::default());
    world.spawn_with((TilemapInstance::new("room", Vec2::ZERO),));
    world
}

fn world(cells: &[(u32, u32, TileIndex)]) -> World {
    world_with(cells, LayerPhysics::DEFAULT)
}

/// An 8 × 8 box whose center is `at`.
fn crate_at(world: &mut World, at: Vec2) -> Entity {
    world.spawn_with(RigidBodyBundle::dynamic(
        Position(at),
        Collider::aabb(Vec2::splat(4.0)),
    ))
}

/// Columns `cols`, every row, of `tile`.
fn columns(cols: std::ops::RangeInclusive<u32>, tile: TileIndex) -> Vec<(u32, u32, TileIndex)> {
    cols.flat_map(|col| (0..20).map(move |row| (col, row, tile)))
        .collect()
}

#[test]
fn a_box_inside_a_wall_moves_to_the_nearest_free_spot() {
    // The wall spans x 80 to 128; the box's center 10 px past its left face.
    let mut world = world(&columns(5..=7, WALL));
    let body = crate_at(&mut world, Vec2::new(90.0, 160.0));
    assert_eq!(
        free_spot(&world, body),
        Some(Vec2::new(74.0, 160.0)),
        "four quarter cells left, clear of the face"
    );
    let near_right = crate_at(&mut world, Vec2::new(120.0, 100.0));
    assert_eq!(free_spot(&world, near_right), Some(Vec2::new(132.0, 100.0)));
}

#[test]
fn a_body_in_contact_within_slop_stays() {
    let mut world = world(&columns(5..=7, WALL));
    let touching = Vec2::new(80.0 - 4.0 + 0.2, 160.0);
    let body = crate_at(&mut world, touching);
    assert_eq!(free_spot(&world, body), Some(touching));
    let apart = crate_at(&mut world, Vec2::new(40.0, 40.0));
    assert_eq!(free_spot(&world, apart), Some(Vec2::new(40.0, 40.0)));
}

#[test]
fn one_way_and_slope_cells_are_not_solid_for_it() {
    let mut world = world(&[(3, 3, LEDGE), (8, 8, RAMP)]);
    let on_ledge = crate_at(&mut world, Vec2::new(56.0, 56.0));
    let in_ramp = crate_at(&mut world, Vec2::new(136.0, 136.0));
    assert_eq!(free_spot(&world, on_ledge), Some(Vec2::new(56.0, 56.0)));
    assert_eq!(free_spot(&world, in_ramp), Some(Vec2::new(136.0, 136.0)));
}

#[test]
fn cells_and_bodies_on_layers_it_does_not_meet_are_free() {
    let apart = LayerPhysics {
        layers: 1 << 3,
        mask: 1 << 3,
    };
    let mut world = world_with(&columns(5..=7, WALL), apart);
    let mut body = RigidBodyBundle::dynamic(
        Position(Vec2::new(90.0, 160.0)),
        Collider::aabb(Vec2::splat(4.0)),
    );
    body.body.mask = 1;
    let ghost = world.spawn_with(body);
    assert_eq!(
        free_spot(&world, ghost),
        Some(Vec2::new(90.0, 160.0)),
        "its mask misses the wall's layer"
    );

    let mut pillar = RigidBodyBundle::r#static(
        Position(Vec2::new(300.0, 40.0)),
        Collider::aabb(Vec2::splat(8.0)),
    );
    pillar.body.layers = 1 << 3;
    pillar.body.mask = 1 << 3;
    world.spawn_with(pillar);
    world.spawn_with(RigidBodyBundle::r#static(
        Position(Vec2::new(300.0, 200.0)),
        Collider::circle(8.0),
    ));
    let beside_pillar = crate_at(&mut world, Vec2::new(300.0, 40.0));
    assert_eq!(
        free_spot(&world, beside_pillar),
        Some(Vec2::new(300.0, 40.0)),
        "a static body on a layer it does not meet"
    );
    let in_post = crate_at(&mut world, Vec2::new(300.0, 200.0));
    let moved = free_spot(&world, in_post).unwrap();
    assert_ne!(moved, Vec2::new(300.0, 200.0), "a static body it meets");
    assert!(moved.distance(Vec2::new(300.0, 200.0)) <= 16.0);
}

#[test]
fn a_collider_without_a_body_is_a_solid() {
    // The step reads a collider without a `RigidBody` as a static wall in
    // layer 0 meeting every layer, so a crate inside one moves out of it as
    // out of a static body's, whichever was spawned first; a body whose
    // mask leaves out layer 0 stays.
    let at = Vec2::new(100.0, 100.0);
    for crate_first in [false, true] {
        for bare in [false, true] {
            let mut world = world(&[]);
            let spawn_wall = |world: &mut World| {
                let collider = Collider::aabb(Vec2::splat(16.0));
                if bare {
                    world.spawn_with((Position(at), collider));
                } else {
                    world.spawn_with(RigidBodyBundle::r#static(Position(at), collider));
                }
            };
            if !crate_first {
                spawn_wall(&mut world);
            }
            let body = crate_at(&mut world, at);
            if crate_first {
                spawn_wall(&mut world);
            }
            assert_eq!(
                free_spot(&world, body),
                Some(Vec2::new(100.0, 80.0)),
                "bare {bare}, crate first {crate_first}"
            );
        }
    }
    let mut world = world(&[]);
    world.spawn_with((Position(at), Collider::aabb(Vec2::splat(16.0))));
    let mut ghost = RigidBodyBundle::dynamic(Position(at), Collider::aabb(Vec2::splat(4.0)));
    ghost.body.mask = 1 << 3;
    let ghost = world.spawn_with(ghost);
    assert_eq!(
        free_spot(&world, ghost),
        Some(at),
        "its mask leaves out layer 0"
    );
}

#[test]
fn no_free_spot_within_eight_cells_leaves_it_and_says_so() {
    let mut world = world(&columns(0..=19, WALL));
    let body = crate_at(&mut world, Vec2::new(160.0, 160.0));
    assert_eq!(free_spot(&world, body), None);
    assert_eq!(
        world.get::<Position>(body),
        Some(&Position(Vec2::new(160.0, 160.0))),
        "a read: it moves nothing"
    );
}

#[test]
fn ties_go_up_then_right_down_left() {
    // The box at the center of one wall cell: 12 px to each face.
    let mut cells = vec![(10, 10, WALL)];
    let expected = [
        Vec2::new(168.0, 156.0),
        Vec2::new(180.0, 168.0),
        Vec2::new(168.0, 180.0),
        Vec2::new(156.0, 168.0),
    ];
    let blocks = [(10, 9), (11, 10), (10, 11)];
    for (turn, spot) in expected.into_iter().enumerate() {
        let mut world = world(&cells);
        let body = crate_at(&mut world, Vec2::new(168.0, 168.0));
        assert_eq!(free_spot(&world, body), Some(spot), "turn {turn}");
        if let Some(&(col, row)) = blocks.get(turn) {
            cells.push((col, row, WALL));
        }
    }
}
