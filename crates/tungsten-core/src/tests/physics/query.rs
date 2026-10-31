use super::*;
use crate::assets::tilemap::TerrainProps;
use crate::assets::{LayerKind, TilemapData, TilemapInstance, TilemapLayer, TilemapRegistry};
use crate::physics::components::{Collider, Position, RigidBodyBundle, Velocity};
use crate::physics::physics_step;
use crate::time::Time;

fn world() -> World {
    let mut world = World::new();
    let mut time = Time::new();
    time.advance_frame(1.0 / 60.0);
    world.insert_resource(time);
    world.insert_resource(PhysicsConfig::default());
    world.insert_resource(TilemapRegistry::new());
    world
}

fn wall(world: &mut World, center: Vec2, half: Vec2) -> Entity {
    world.spawn_with(RigidBodyBundle::r#static(
        Position(center),
        Collider::aabb(half),
    ))
}

fn post(world: &mut World, center: Vec2, radius: f32) -> Entity {
    world.spawn_with(RigidBodyBundle::r#static(
        Position(center),
        Collider::circle(radius),
    ))
}

/// A row of `width` solid 16 px tiles under an empty one: the floor spans
/// x 0 to `16 · width`, its top face at y 16.
fn floor(world: &mut World, width: u32) {
    let mut tiles = vec![-1; width as usize];
    tiles.extend(std::iter::repeat_n(0, width as usize));
    world.get_resource_mut::<TilemapRegistry>().unwrap().insert(
        "floor".into(),
        TilemapData {
            tile_width: 16,
            tile_height: 16,
            width,
            height: 2,
            tileset: vec!["solid".into()],
            object_layers: Vec::new(),
            terrain: TerrainProps::default(),
            layers: vec![TilemapLayer {
                name: "solid".into(),
                kind: LayerKind::Collision,
                tiles,
            }],
        },
    );
    let map = world.spawn();
    world.insert(map, TilemapInstance::new("floor", Vec2::ZERO));
}

fn all() -> SpatialFilter {
    SpatialFilter::new()
}

fn hit_parts(hit: SpatialHit) -> (Option<Entity>, Vec2, Vec2, f32) {
    (hit.entity, hit.point, hit.normal, hit.distance)
}

fn assert_close(actual: Vec2, expected: Vec2) {
    assert!(
        (actual - expected).length() < 1.0e-3,
        "{actual:?} is not {expected:?}"
    );
}

#[test]
fn ray_hits_a_box_with_point_normal_and_distance() {
    let mut world = world();
    let wall = wall(&mut world, Vec2::new(100.0, 0.0), Vec2::new(10.0, 50.0));
    physics_step(&mut world);
    let hit = cast_ray(&world, Vec2::ZERO, Vec2::X, 500.0, &all()).unwrap();
    assert_eq!(
        hit_parts(hit),
        (Some(wall), Vec2::new(90.0, 0.0), Vec2::NEG_X, 90.0)
    );
    // A direction that is not unit length is normalized.
    let hit = cast_ray(
        &world,
        Vec2::new(100.0, -200.0),
        Vec2::new(0.0, 5.0),
        500.0,
        &all(),
    )
    .unwrap();
    assert_eq!(
        hit_parts(hit),
        (Some(wall), Vec2::new(100.0, -50.0), Vec2::NEG_Y, 150.0)
    );
    let hit = cast_ray(&world, Vec2::new(80.0, -10.0), Vec2::ONE, 500.0, &all()).unwrap();
    assert_close(hit.point, Vec2::new(90.0, 0.0));
    assert_eq!(hit.normal, Vec2::NEG_X);
    assert!((hit.distance - 10.0 * std::f32::consts::SQRT_2).abs() < 1.0e-3);
}

#[test]
fn ray_hits_a_circle_with_point_normal_and_distance() {
    let mut world = world();
    let post = post(&mut world, Vec2::new(100.0, 0.0), 10.0);
    physics_step(&mut world);
    let hit = cast_ray(&world, Vec2::ZERO, Vec2::X, 500.0, &all()).unwrap();
    assert_eq!(
        hit_parts(hit),
        (Some(post), Vec2::new(90.0, 0.0), Vec2::NEG_X, 90.0)
    );
    let hit = cast_ray(&world, Vec2::new(100.0, 100.0), Vec2::NEG_Y, 500.0, &all()).unwrap();
    assert_eq!(
        hit_parts(hit),
        (Some(post), Vec2::new(100.0, 10.0), Vec2::Y, 90.0)
    );
}

#[test]
fn ray_hits_a_tile_and_never_an_internal_face() {
    let mut world = world();
    floor(&mut world, 10);
    physics_step(&mut world);
    let hit = cast_ray(&world, Vec2::new(40.0, -50.0), Vec2::Y, 500.0, &all()).unwrap();
    assert_eq!(
        hit_parts(hit),
        (None, Vec2::new(40.0, 16.0), Vec2::NEG_Y, 66.0)
    );
    let hit = cast_ray(&world, Vec2::new(-50.0, 24.0), Vec2::X, 500.0, &all()).unwrap();
    assert_eq!(
        hit_parts(hit),
        (None, Vec2::new(0.0, 24.0), Vec2::NEG_X, 50.0)
    );
    // Straight down a seam: the closed tiles leave no gap between them.
    let hit = cast_ray(&world, Vec2::new(32.0, -50.0), Vec2::Y, 500.0, &all()).unwrap();
    assert_eq!(
        hit_parts(hit),
        (None, Vec2::new(32.0, 16.0), Vec2::NEG_Y, 66.0)
    );
    // From inside the floor, every face ahead is internal or behind.
    assert_eq!(
        cast_ray(&world, Vec2::new(40.0, 24.0), Vec2::X, 500.0, &all()),
        None
    );
    // Along the floor's surface from above a tile: no seam catches it.
    assert_eq!(
        cast_ray(&world, Vec2::new(5.0, 16.0), Vec2::X, 100.0, &all()),
        None
    );
}

#[test]
fn ray_stops_at_max_distance() {
    let mut world = world();
    wall(&mut world, Vec2::new(100.0, 0.0), Vec2::splat(10.0));
    physics_step(&mut world);
    assert_eq!(cast_ray(&world, Vec2::ZERO, Vec2::X, 89.9, &all()), None);
    let hit = cast_ray(&world, Vec2::ZERO, Vec2::X, 90.0, &all()).unwrap();
    assert_eq!(hit.distance, 90.0, "max_distance is inclusive");
    let hit = cast_shape(
        &world,
        Shape::Circle { radius: 5.0 },
        Vec2::ZERO,
        Vec2::X,
        85.0,
        &all(),
    )
    .unwrap();
    assert_eq!(hit.distance, 85.0);
    assert_eq!(
        cast_shape(
            &world,
            Shape::Circle { radius: 5.0 },
            Vec2::ZERO,
            Vec2::X,
            84.0,
            &all()
        ),
        None
    );
}

#[test]
fn ray_from_inside_a_shape_passes_it() {
    let mut world = world();
    let _inner = wall(&mut world, Vec2::ZERO, Vec2::splat(10.0));
    let _ring = post(&mut world, Vec2::ZERO, 20.0);
    let far = wall(&mut world, Vec2::new(50.0, 0.0), Vec2::splat(10.0));
    physics_step(&mut world);
    let hit = cast_ray(&world, Vec2::ZERO, Vec2::X, 500.0, &all()).unwrap();
    assert_eq!(
        hit_parts(hit),
        (Some(far), Vec2::new(40.0, 0.0), Vec2::NEG_X, 40.0)
    );
    // On the inner box's edge counts as inside it, and on the circle too.
    let hit = cast_ray(&world, Vec2::new(20.0, 0.0), Vec2::X, 500.0, &all()).unwrap();
    assert_eq!((hit.entity, hit.distance), (Some(far), 20.0));
}

#[test]
fn zero_or_non_finite_cast_inputs_return_none() {
    let mut world = world();
    wall(&mut world, Vec2::new(100.0, 0.0), Vec2::splat(10.0));
    physics_step(&mut world);
    let circle = Shape::Circle { radius: 5.0 };
    for direction in [
        Vec2::ZERO,
        Vec2::new(f32::NAN, 1.0),
        Vec2::new(f32::INFINITY, 0.0),
    ] {
        assert_eq!(cast_ray(&world, Vec2::ZERO, direction, 500.0, &all()), None);
        assert_eq!(
            cast_shape(&world, circle, Vec2::ZERO, direction, 500.0, &all()),
            None
        );
    }
    for max_distance in [-1.0, f32::NAN, f32::INFINITY] {
        assert_eq!(
            cast_ray(&world, Vec2::ZERO, Vec2::X, max_distance, &all()),
            None
        );
        assert_eq!(nearest(&world, Vec2::ZERO, max_distance, &all()), None);
    }
    assert_eq!(
        cast_ray(&world, Vec2::new(f32::NAN, 0.0), Vec2::X, 500.0, &all()),
        None
    );
    assert_eq!(
        cast_shape(
            &world,
            Shape::Circle { radius: -1.0 },
            Vec2::ZERO,
            Vec2::X,
            500.0,
            &all()
        ),
        None
    );
    let mut out = vec![None];
    overlap(
        &world,
        Shape::Circle { radius: f32::NAN },
        Vec2::new(100.0, 0.0),
        &all(),
        &mut out,
    );
    assert!(out.is_empty());
}

#[test]
fn box_cast_hits_boxes_circles_and_tiles() {
    let mut world = world();
    let wall = wall(&mut world, Vec2::new(100.0, 0.0), Vec2::new(10.0, 50.0));
    let post = post(&mut world, Vec2::new(100.0, 100.0), 10.0);
    floor(&mut world, 4);
    physics_step(&mut world);
    let shape = Shape::Aabb {
        half_extents: Vec2::splat(5.0),
    };
    let hit = cast_shape(&world, shape, Vec2::ZERO, Vec2::X, 500.0, &all()).unwrap();
    assert_eq!(
        hit_parts(hit),
        (Some(wall), Vec2::new(90.0, 0.0), Vec2::NEG_X, 85.0)
    );
    let hit = cast_shape(&world, shape, Vec2::new(0.0, 100.0), Vec2::X, 500.0, &all()).unwrap();
    assert_eq!(
        hit_parts(hit),
        (Some(post), Vec2::new(90.0, 100.0), Vec2::NEG_X, 85.0)
    );
    let small = Shape::Aabb {
        half_extents: Vec2::splat(4.0),
    };
    let hit = cast_shape(
        &world,
        small,
        Vec2::new(40.0, -50.0),
        Vec2::Y,
        500.0,
        &all(),
    )
    .unwrap();
    assert_eq!(
        hit_parts(hit),
        (None, Vec2::new(40.0, 16.0), Vec2::NEG_Y, 62.0)
    );
}

#[test]
fn circle_cast_hits_boxes_circles_corners_and_tiles() {
    let mut world = world();
    let wall = wall(&mut world, Vec2::new(100.0, 0.0), Vec2::new(10.0, 50.0));
    let post = post(&mut world, Vec2::new(100.0, 200.0), 10.0);
    floor(&mut world, 4);
    physics_step(&mut world);
    let shape = Shape::Circle { radius: 5.0 };
    let hit = cast_shape(&world, shape, Vec2::ZERO, Vec2::X, 500.0, &all()).unwrap();
    assert_eq!(
        hit_parts(hit),
        (Some(wall), Vec2::new(90.0, 0.0), Vec2::NEG_X, 85.0)
    );
    // Past the top-left corner's face: the rounded corner meets it.
    let hit = cast_shape(&world, shape, Vec2::new(0.0, -53.0), Vec2::X, 500.0, &all()).unwrap();
    assert_eq!(hit.entity, Some(wall));
    assert!((hit.distance - 86.0).abs() < 1.0e-3, "{}", hit.distance);
    assert_close(hit.normal, Vec2::new(-0.8, -0.6));
    assert_close(hit.point, Vec2::new(90.0, -50.0));
    let hit = cast_shape(&world, shape, Vec2::new(0.0, 200.0), Vec2::X, 500.0, &all()).unwrap();
    assert_eq!(
        hit_parts(hit),
        (Some(post), Vec2::new(90.0, 200.0), Vec2::NEG_X, 85.0)
    );
    let small = Shape::Circle { radius: 4.0 };
    let hit = cast_shape(
        &world,
        small,
        Vec2::new(40.0, -50.0),
        Vec2::Y,
        500.0,
        &all(),
    )
    .unwrap();
    assert_eq!(
        hit_parts(hit),
        (None, Vec2::new(40.0, 16.0), Vec2::NEG_Y, 62.0)
    );
}

#[test]
fn cast_that_starts_overlapping_stops_moving_in_and_passes_moving_out() {
    let mut world = world();
    let near = wall(&mut world, Vec2::ZERO, Vec2::splat(10.0));
    let far = wall(&mut world, Vec2::new(50.0, 0.0), Vec2::splat(10.0));
    floor(&mut world, 8);
    physics_step(&mut world);
    let shape = Shape::Aabb {
        half_extents: Vec2::splat(5.0),
    };
    // 3 px into `near`'s right face.
    let start = Vec2::new(12.0, 0.0);
    let hit = cast_shape(&world, shape, start, Vec2::NEG_X, 100.0, &all()).unwrap();
    assert_eq!(
        hit_parts(hit),
        (Some(near), Vec2::new(7.0, 0.0), Vec2::X, 0.0)
    );
    let hit = cast_shape(&world, shape, start, Vec2::X, 100.0, &all()).unwrap();
    assert_eq!((hit.entity, hit.distance), (Some(far), 23.0));
    assert_eq!(
        cast_shape(&world, shape, start, Vec2::NEG_Y, 30.0, &all()),
        None,
        "moving along the push-out face passes"
    );
    // A circle 0.25 px into the floor: down stops, up and along pass.
    let circle = Shape::Circle { radius: 6.0 };
    let sunk = Vec2::new(70.0, 10.25);
    let hit = cast_shape(&world, circle, sunk, Vec2::Y, 30.0, &all()).unwrap();
    assert_eq!(
        (hit.entity, hit.normal, hit.distance),
        (None, Vec2::NEG_Y, 0.0)
    );
    assert_eq!(
        cast_shape(&world, circle, sunk, Vec2::NEG_Y, 30.0, &all()),
        None
    );
    assert_eq!(
        cast_shape(&world, circle, sunk, Vec2::X, 30.0, &all()),
        None
    );
}

#[test]
fn cast_along_a_row_of_tiles_catches_on_no_seam() {
    let mut world = world();
    floor(&mut world, 10);
    physics_step(&mut world);
    let boxed = Shape::Aabb {
        half_extents: Vec2::splat(6.0),
    };
    let circle = Shape::Circle { radius: 6.0 };
    // Sunk 0.25 px as a resting body is, exactly touching, and hovering.
    for y in [10.25, 10.0, 9.0] {
        for shape in [boxed, circle] {
            for (start, direction) in [(20.0, Vec2::X), (140.0, Vec2::NEG_X)] {
                assert_eq!(
                    cast_shape(&world, shape, Vec2::new(start, y), direction, 100.0, &all()),
                    None,
                    "{shape:?} at y {y} from x {start} caught a seam"
                );
            }
        }
    }
    // Entity colliders keep every face, so the same box catches on them.
    let mut world = self::world();
    for i in 0..10 {
        wall(
            &mut world,
            Vec2::new(8.0 + 16.0 * i as f32, 24.0),
            Vec2::splat(8.0),
        );
    }
    physics_step(&mut world);
    let hit = cast_shape(
        &world,
        boxed,
        Vec2::new(20.0, 10.25),
        Vec2::X,
        100.0,
        &all(),
    )
    .unwrap();
    assert_eq!((hit.normal, hit.distance), (Vec2::NEG_X, 6.0));
}

#[test]
fn overlap_is_strict_and_in_index_order() {
    let mut world = world();
    let resting = wall(&mut world, Vec2::new(8.0, 8.0), Vec2::splat(8.0));
    let ball = post(&mut world, Vec2::new(30.0, 8.0), 6.0);
    floor(&mut world, 4);
    physics_step(&mut world);
    let mut out = Vec::new();
    let probe = Shape::Aabb {
        half_extents: Vec2::splat(4.0),
    };
    overlap(&world, probe, Vec2::new(8.0, 14.0), &all(), &mut out);
    assert_eq!(out, [Some(resting), None]);
    // Touching the floor's top face is not overlapping it.
    overlap(&world, probe, Vec2::new(8.0, 12.0), &all(), &mut out);
    assert_eq!(out, [Some(resting)]);
    // Bodies in spawn order, then tiles in map order.
    overlap(
        &world,
        Shape::Circle { radius: 12.0 },
        Vec2::new(20.0, 12.0),
        &all(),
        &mut out,
    );
    assert_eq!(out, [Some(resting), Some(ball), None, None]);
    // A shape inside a tile overlaps it, faces or none.
    overlap(
        &world,
        Shape::Circle { radius: 1.0 },
        Vec2::new(40.0, 24.0),
        &all(),
        &mut out,
    );
    assert_eq!(out, [None]);
}

#[test]
fn filter_excludes_bodies_and_tiles() {
    let mut world = world();
    let near = wall(&mut world, Vec2::new(50.0, 0.0), Vec2::splat(5.0));
    let far = wall(&mut world, Vec2::new(100.0, 0.0), Vec2::splat(5.0));
    floor(&mut world, 12);
    physics_step(&mut world);
    let hit = cast_ray(&world, Vec2::ZERO, Vec2::X, 500.0, &all()).unwrap();
    assert_eq!(hit.entity, Some(near));
    let hit = cast_ray(&world, Vec2::ZERO, Vec2::X, 500.0, &all().excluding(near)).unwrap();
    assert_eq!(hit.entity, Some(far));
    let down = Vec2::new(50.0, -20.0);
    let hit = cast_ray(&world, down, Vec2::Y, 500.0, &all()).unwrap();
    assert_eq!(hit.entity, Some(near));
    let tiles = all().with_bodies(false);
    let hit = cast_ray(&world, down, Vec2::Y, 500.0, &tiles).unwrap();
    assert_eq!((hit.entity, hit.point), (None, Vec2::new(50.0, 16.0)));
    let bodies = all().with_tiles(false);
    assert_eq!(
        cast_ray(&world, Vec2::new(40.0, 24.0), Vec2::NEG_Y, 10.0, &tiles),
        None
    );
    let hit = cast_ray(&world, Vec2::new(150.0, 40.0), Vec2::NEG_Y, 500.0, &bodies);
    assert_eq!(hit, None, "past the floor, with tiles off, nothing");
    let mut out = Vec::new();
    let probe = Shape::Circle { radius: 8.0 };
    overlap(&world, probe, Vec2::new(50.0, 10.0), &all(), &mut out);
    assert_eq!(out, [Some(near), None, None]);
    overlap(&world, probe, Vec2::new(50.0, 10.0), &tiles, &mut out);
    assert_eq!(out, [None, None]);
    overlap(&world, probe, Vec2::new(50.0, 10.0), &bodies, &mut out);
    assert_eq!(out, [Some(near)]);
}

#[test]
fn despawned_body_is_skipped() {
    let mut world = world();
    let near = wall(&mut world, Vec2::new(50.0, 0.0), Vec2::splat(10.0));
    let far = wall(&mut world, Vec2::new(100.0, 0.0), Vec2::splat(10.0));
    physics_step(&mut world);
    world.despawn(near);
    let hit = cast_ray(&world, Vec2::ZERO, Vec2::X, 500.0, &all()).unwrap();
    assert_eq!(hit.entity, Some(far));
    assert_eq!(
        nearest(&world, Vec2::new(50.0, 0.0), 500.0, &all())
            .unwrap()
            .entity,
        Some(far)
    );
    let mut out = Vec::new();
    overlap(
        &world,
        Shape::Circle { radius: 5.0 },
        Vec2::new(50.0, 0.0),
        &all(),
        &mut out,
    );
    assert!(out.is_empty());
}

#[test]
fn queries_see_the_last_step() {
    let mut world = world();
    let mover = world.spawn_with(
        RigidBodyBundle::dynamic(Position(Vec2::ZERO), Collider::circle(5.0))
            .with_velocity(Velocity(Vec2::new(600.0, 0.0))),
    );
    let mut out = Vec::new();
    // Before the first step there is nothing to find.
    assert_eq!(nearest(&world, Vec2::ZERO, 100.0, &all()), None);
    assert_eq!(
        cast_ray(&world, Vec2::new(-50.0, 0.0), Vec2::X, 100.0, &all()),
        None
    );
    overlap(
        &world,
        Shape::Circle { radius: 50.0 },
        Vec2::ZERO,
        &all(),
        &mut out,
    );
    assert!(out.is_empty());

    physics_step(&mut world);
    let stepped = world.get::<Position>(mover).unwrap().0.x;
    assert!((stepped - 10.0).abs() < 1.0e-3, "{stepped}");
    // Moved and spawned after the step: still where the step left it, and
    // the newcomer unseen until the next step.
    world.get_mut::<Position>(mover).unwrap().0 = Vec2::new(300.0, 0.0);
    let late = wall(&mut world, Vec2::new(40.0, 0.0), Vec2::splat(5.0));
    let hit = cast_ray(&world, Vec2::new(-50.0, 0.0), Vec2::X, 500.0, &all()).unwrap();
    assert_eq!(hit.entity, Some(mover));
    assert!(
        (hit.distance - (stepped + 45.0)).abs() < 1.0e-3,
        "{}",
        hit.distance
    );

    physics_step(&mut world);
    let hit = cast_ray(&world, Vec2::new(-50.0, 0.0), Vec2::X, 500.0, &all()).unwrap();
    assert_eq!(hit.entity, Some(late));
    let hit = cast_ray(&world, Vec2::new(200.0, 0.0), Vec2::X, 500.0, &all()).unwrap();
    assert_eq!(hit.entity, Some(mover));
    assert!((hit.distance - 105.0).abs() < 1.0e-3, "{}", hit.distance);
}

#[test]
fn nearest_finds_the_closest_surface_with_ties_to_the_lower_index() {
    let mut world = world();
    let wall = wall(&mut world, Vec2::new(100.0, 0.0), Vec2::splat(10.0));
    let post = post(&mut world, Vec2::new(0.0, 100.0), 10.0);
    physics_step(&mut world);
    // Both surfaces lie 90 px away; the wall was spawned first.
    let hit = nearest(&world, Vec2::ZERO, 500.0, &all()).unwrap();
    assert_eq!(
        hit_parts(hit),
        (Some(wall), Vec2::new(90.0, 0.0), Vec2::NEG_X, 90.0)
    );
    assert_eq!(nearest(&world, Vec2::ZERO, 89.0, &all()), None);
    let hit = nearest(&world, Vec2::new(0.0, 60.0), 500.0, &all()).unwrap();
    assert_eq!(
        hit_parts(hit),
        (Some(post), Vec2::new(0.0, 90.0), Vec2::NEG_Y, 30.0)
    );
    // Inside a shape: distance 0, the point itself and no normal.
    let inside = Vec2::new(104.0, 3.0);
    let hit = nearest(&world, inside, 500.0, &all()).unwrap();
    assert_eq!(hit_parts(hit), (Some(wall), inside, Vec2::ZERO, 0.0));
}

#[test]
fn identical_worlds_answer_alike() {
    use crate::Pcg32;

    let run = || {
        let mut world = world();
        let mut rng = Pcg32::seeded(0x49);
        world.get_resource_mut::<PhysicsConfig>().unwrap().gravity = Vec2::new(0.0, 900.0);
        floor(&mut world, 12);
        for _ in 0..40 {
            let position = Vec2::new(rng.next_range(10.0, 180.0), rng.next_range(-120.0, 0.0));
            let collider = if rng.next_range(0.0, 1.0) < 0.5 {
                Collider::circle(rng.next_range(3.0, 8.0))
            } else {
                Collider::aabb(Vec2::splat(rng.next_range(3.0, 8.0)))
            };
            world.spawn_with(RigidBodyBundle::dynamic(Position(position), collider));
        }
        for _ in 0..30 {
            physics_step(&mut world);
        }
        let mut answers = Vec::new();
        let mut out = Vec::new();
        for i in 0..20 {
            let origin = Vec2::new(10.0 * i as f32, -150.0);
            let direction = Vec2::new(rng.next_range(-1.0, 1.0), 1.0);
            answers.push(format!(
                "{:?}",
                cast_ray(&world, origin, direction, 400.0, &all())
            ));
            let shape = Shape::Circle { radius: 4.0 };
            answers.push(format!(
                "{:?}",
                cast_shape(&world, shape, origin, direction, 400.0, &all())
            ));
            answers.push(format!("{:?}", nearest(&world, origin, 400.0, &all())));
            overlap(
                &world,
                Shape::Circle { radius: 30.0 },
                origin + Vec2::Y * 150.0,
                &all(),
                &mut out,
            );
            answers.push(format!("{out:?}"));
        }
        answers
    };
    let first = run();
    assert!(first.iter().any(|answer| answer.starts_with("Some")));
    assert_eq!(first, run());
}

#[test]
fn index_is_built_at_most_once_per_step() {
    let builds = |world: &World| {
        world
            .get_resource::<SpatialIndex>()
            .unwrap()
            .0
            .borrow()
            .builds
    };
    let mut world = world();
    wall(&mut world, Vec2::new(100.0, 0.0), Vec2::splat(10.0));
    floor(&mut world, 4);
    physics_step(&mut world);
    assert_eq!(builds(&world), 0, "a step without sensors builds nothing");
    let mut out = Vec::new();
    for _ in 0..3 {
        let _ = cast_ray(&world, Vec2::ZERO, Vec2::X, 500.0, &all());
        let _ = nearest(&world, Vec2::ZERO, 500.0, &all());
        overlap(
            &world,
            Shape::Circle { radius: 50.0 },
            Vec2::ZERO,
            &all(),
            &mut out,
        );
    }
    assert_eq!(builds(&world), 1);
    // A paused clock runs no step and leaves the index alone.
    world.get_resource_mut::<Time>().unwrap().advance_frame(0.0);
    physics_step(&mut world);
    let _ = cast_ray(&world, Vec2::ZERO, Vec2::X, 500.0, &all());
    assert_eq!(builds(&world), 1);
    world
        .get_resource_mut::<Time>()
        .unwrap()
        .advance_frame(1.0 / 60.0);
    physics_step(&mut world);
    physics_step(&mut world);
    let _ = cast_ray(&world, Vec2::ZERO, Vec2::X, 500.0, &all());
    let _ = cast_ray(&world, Vec2::ZERO, Vec2::X, 500.0, &all());
    assert_eq!(builds(&world), 2);
}

#[test]
fn queries_see_a_sensor_only_with_sensors() {
    let builds = |world: &World| {
        world
            .get_resource::<SpatialIndex>()
            .unwrap()
            .0
            .borrow()
            .builds
    };
    let mut world = world();
    let sensor = world.spawn_with(RigidBodyBundle::sensor(
        Position::new(50.0, 0.0),
        Collider::aabb(Vec2::splat(10.0)),
    ));
    physics_step(&mut world);
    assert_eq!(builds(&world), 1, "a step with sensors builds the index");
    let sensors = all().with_sensors(true);
    assert_eq!(cast_ray(&world, Vec2::ZERO, Vec2::X, 500.0, &all()), None);
    let hit = cast_ray(&world, Vec2::ZERO, Vec2::X, 500.0, &sensors).unwrap();
    assert_eq!(
        hit_parts(hit),
        (Some(sensor), Vec2::new(40.0, 0.0), Vec2::NEG_X, 40.0)
    );
    assert_eq!(
        cast_ray(
            &world,
            Vec2::ZERO,
            Vec2::X,
            500.0,
            &sensors.excluding(sensor)
        ),
        None
    );
    let shape = Shape::Circle { radius: 4.0 };
    assert_eq!(
        cast_shape(&world, shape, Vec2::ZERO, Vec2::X, 500.0, &all()),
        None
    );
    let hit = cast_shape(&world, shape, Vec2::ZERO, Vec2::X, 500.0, &sensors).unwrap();
    assert_eq!((hit.entity, hit.distance), (Some(sensor), 36.0));
    let mut out = Vec::new();
    overlap(&world, shape, Vec2::new(45.0, 0.0), &all(), &mut out);
    assert!(out.is_empty());
    overlap(&world, shape, Vec2::new(45.0, 0.0), &sensors, &mut out);
    assert_eq!(out, [Some(sensor)]);
    assert_eq!(nearest(&world, Vec2::ZERO, 500.0, &all()), None);
    assert_eq!(
        nearest(&world, Vec2::ZERO, 500.0, &sensors).unwrap().entity,
        Some(sensor)
    );
    assert_eq!(builds(&world), 1, "the step's index served every query");
}

/// A static box at `center` in `layers`.
fn layered_wall(world: &mut World, center: Vec2, layers: u32) -> Entity {
    world.spawn_with(
        RigidBodyBundle::r#static(Position(center), Collider::aabb(Vec2::splat(8.0)))
            .with_layers(layers),
    )
}

#[test]
fn layers_casts_skip_bodies_sensors_and_tiles_outside_the_mask() {
    // Down the line x 40: a sensor and a box in layer 2, then the floor.
    let mut world = world();
    floor(&mut world, 10);
    let boxed = layered_wall(&mut world, Vec2::new(40.0, -40.0), 1 << 2);
    let sensor = world.spawn_with(
        RigidBodyBundle::sensor(Position::new(40.0, -80.0), Collider::aabb(Vec2::splat(8.0)))
            .with_layers(1 << 2),
    );
    physics_step(&mut world);
    let origin = Vec2::new(40.0, -200.0);
    let shape = Shape::Circle { radius: 3.0 };
    let ray = |filter: &SpatialFilter| {
        cast_ray(&world, origin, Vec2::Y, 500.0, filter).map(|hit| hit.entity)
    };
    let swept = |filter: &SpatialFilter| {
        cast_shape(&world, shape, origin, Vec2::Y, 500.0, filter).map(|hit| hit.entity)
    };
    let sensors = all().with_sensors(true);
    for cast in [
        &ray as &dyn Fn(&SpatialFilter) -> Option<Option<Entity>>,
        &swept,
    ] {
        assert_eq!(cast(&sensors), Some(Some(sensor)));
        assert_eq!(cast(&sensors.with_mask(1 << 2)), Some(Some(sensor)));
        assert_eq!(cast(&all().with_mask(1 << 2)), Some(Some(boxed)));
        assert_eq!(
            cast(&sensors.with_mask(RigidBody::DEFAULT_LAYERS)),
            Some(None)
        );
        assert_eq!(cast(&sensors.with_mask(1 << 5)), None);
        assert_eq!(cast(&sensors.with_mask(0)), None);
        // A later mask replaces an earlier one.
        assert_eq!(
            cast(&sensors.with_mask(0).with_mask(u32::MAX)),
            Some(Some(sensor))
        );
    }
}

#[test]
fn layers_overlap_and_nearest_skip_what_the_mask_lacks() {
    let mut world = world();
    floor(&mut world, 4);
    let plain = layered_wall(
        &mut world,
        Vec2::new(10.0, -20.0),
        RigidBody::DEFAULT_LAYERS,
    );
    let layered = layered_wall(&mut world, Vec2::new(30.0, -20.0), 1 << 2);
    let sensor = world.spawn_with(
        RigidBodyBundle::sensor(Position::new(50.0, -20.0), Collider::aabb(Vec2::splat(8.0)))
            .with_layers(1 << 2),
    );
    physics_step(&mut world);
    let big = Shape::Aabb {
        half_extents: Vec2::splat(60.0),
    };
    let sensors = all().with_sensors(true);
    let mut out = Vec::new();
    let mut overlaps = |filter: &SpatialFilter| {
        overlap(&world, big, Vec2::new(30.0, 0.0), filter, &mut out);
        out.clone()
    };
    assert_eq!(
        overlaps(&sensors),
        [
            Some(plain),
            Some(layered),
            None,
            None,
            None,
            None,
            Some(sensor)
        ]
    );
    assert_eq!(
        overlaps(&sensors.with_mask(1 << 2)),
        [Some(layered), Some(sensor)]
    );
    assert_eq!(
        overlaps(&sensors.with_mask(RigidBody::DEFAULT_LAYERS)),
        [Some(plain), None, None, None, None]
    );
    assert!(overlaps(&sensors.with_mask(1 << 7)).is_empty());
    let near = |point: Vec2, filter: &SpatialFilter| {
        nearest(&world, point, 500.0, filter).map(|hit| hit.entity)
    };
    // Inside the layered box; the plain box is 12 px away, the floor 36.
    let inside = Vec2::new(30.0, -20.0);
    assert_eq!(near(inside, &all()), Some(Some(layered)));
    assert_eq!(near(inside, &all().with_mask(1 << 2)), Some(Some(layered)));
    assert_eq!(
        near(inside, &all().with_mask(RigidBody::DEFAULT_LAYERS)),
        Some(Some(plain))
    );
    assert_eq!(
        near(
            Vec2::new(30.0, 40.0),
            &all().with_mask(RigidBody::DEFAULT_LAYERS)
        ),
        Some(None)
    );
    assert_eq!(near(inside, &all().with_mask(1 << 7)), None);
    // Inside the sensor, which only a filter with sensors sees.
    let in_sensor = Vec2::new(50.0, -20.0);
    assert_eq!(
        near(in_sensor, &sensors.with_mask(1 << 2)),
        Some(Some(sensor))
    );
    assert_eq!(
        near(in_sensor, &all().with_mask(1 << 2)),
        Some(Some(layered))
    );
}

#[test]
fn filters_see_tiles_by_their_collision_layers() {
    // One row of 16 px cells in three collision layers: `low`'s tile at
    // column 2 in layer 1, `both`'s at column 5 in layers 0 and 1, `plain`'s
    // at column 7 at the defaults.
    let mut world = world();
    let mut map = TilemapData::new(16, 16, 8, 1);
    let solid = map.add_tile("solid");
    for (name, col, layers) in [("low", 2, 1 << 1), ("both", 5, 0b11), ("plain", 7, 1)] {
        let layer = map.add_layer(name, LayerKind::Collision);
        map.set_tile(layer, col, 0, solid);
        if layers != 1 {
            map.set_layer_physics(
                layer,
                crate::assets::LayerPhysics {
                    layers,
                    mask: RigidBody::ALL_LAYERS,
                },
            );
        }
    }
    world
        .resource_mut::<TilemapRegistry>()
        .insert("row".into(), map);
    let instance = world.spawn();
    world.insert(instance, TilemapInstance::new("row", Vec2::ZERO));
    physics_step(&mut world);

    let ray = |from: f32, filter: &SpatialFilter| {
        cast_ray(&world, Vec2::new(from, 8.0), Vec2::X, 500.0, filter)
            .map(|hit| (hit.point.x, hit.layers, hit.entity))
    };
    let low = Some((32.0, 1 << 1, None));
    let both = Some((80.0, 0b11, None));
    let plain = Some((112.0, 1, None));
    assert_eq!(ray(-10.0, &all()), low);
    assert_eq!(ray(-10.0, &all().with_mask(1)), both, "layer 0's");
    assert_eq!(ray(-10.0, &all().with_mask(1 << 1)), low);
    assert_eq!(ray(50.0, &all().with_mask(1 << 1)), both, "layer 1's");
    assert_eq!(ray(-10.0, &all().with_mask(1 << 2)), None);
    assert_eq!(
        ray(-10.0, &all().without_layers(1 << 1)),
        plain,
        "a tile in layers 0 and 1 is skipped without layer 1"
    );
    assert_eq!(ray(-10.0, &all().without_layers(1)), low);
    assert_eq!(ray(50.0, &all().without_layers(1)), None);
    assert_eq!(
        ray(-10.0, &all().with_mask(1 << 1).without_layers(1)),
        low,
        "the mask and the skipped layers both hold"
    );
    assert_eq!(ray(50.0, &all().with_mask(1 << 1).without_layers(1)), None);
    // A later call replaces the earlier skipped layers.
    assert_eq!(
        ray(-10.0, &all().without_layers(1 << 1).without_layers(0)),
        low
    );
    let swept = cast_shape(
        &world,
        Shape::Circle { radius: 3.0 },
        Vec2::new(-10.0, 8.0),
        Vec2::X,
        500.0,
        &all().without_layers(1 << 1),
    )
    .unwrap();
    assert_eq!((swept.point.x, swept.layers), (112.0, 1));
    let mut out = Vec::new();
    let mut overlaps = |filter: &SpatialFilter| {
        overlap(
            &world,
            Shape::Aabb {
                half_extents: Vec2::new(64.0, 4.0),
            },
            Vec2::new(64.0, 8.0),
            filter,
            &mut out,
        );
        out.len()
    };
    assert_eq!(overlaps(&all()), 3);
    assert_eq!(overlaps(&all().with_mask(1)), 2);
    assert_eq!(overlaps(&all().without_layers(1)), 1);
    assert_eq!(overlaps(&all().without_layers(1 << 1)), 1);
    let near = nearest(
        &world,
        Vec2::new(64.0, 8.0),
        500.0,
        &all().without_layers(1),
    )
    .unwrap();
    assert_eq!((near.point.x, near.layers), (48.0, 1 << 1));
}

#[test]
fn a_hit_names_the_tile_cell_it_met() {
    // A 4 × 3 map at (100, 50): `decor`, then two collision layers, `low`
    // with a tile at column 1 of row 2 and `high` one at column 3 of row 0.
    let mut world = world();
    let mut data = TilemapData::new(16, 16, 4, 3);
    let solid = data.add_tile("solid");
    let decor = data.add_layer("decor", LayerKind::Render);
    data.fill(decor, solid);
    let low = data.add_layer("low", LayerKind::Collision);
    let high = data.add_layer("high", LayerKind::Collision);
    data.set_tile(low, 1, 2, solid);
    data.set_tile(high, 3, 0, solid);
    world
        .resource_mut::<TilemapRegistry>()
        .insert("cells".into(), data);
    let map = world.spawn();
    world.insert(map, TilemapInstance::new("cells", Vec2::new(100.0, 50.0)));
    let post = wall(&mut world, Vec2::new(300.0, 90.0), Vec2::splat(4.0));
    physics_step(&mut world);
    let cell = |layer: usize, col: u32, row: u32| {
        Some(TileHit {
            map,
            layer,
            col,
            row,
        })
    };

    let down = cast_ray(&world, Vec2::new(124.0, 0.0), Vec2::Y, 500.0, &all()).unwrap();
    assert_eq!(
        (down.point, down.tile),
        (Vec2::new(124.0, 82.0), cell(1, 1, 2))
    );
    let across = cast_ray(&world, Vec2::new(0.0, 58.0), Vec2::X, 500.0, &all()).unwrap();
    assert_eq!(across.tile, cell(2, 3, 0));
    let swept = cast_shape(
        &world,
        Shape::Circle { radius: 2.0 },
        Vec2::new(124.0, 0.0),
        Vec2::Y,
        500.0,
        &all(),
    )
    .unwrap();
    assert_eq!(swept.tile, cell(1, 1, 2));
    // A cast starting inside the tile, moving into it, stops there at once.
    let inside = cast_shape(
        &world,
        Shape::Aabb {
            half_extents: Vec2::splat(2.0),
        },
        Vec2::new(124.0, 83.0),
        Vec2::Y,
        10.0,
        &all(),
    )
    .unwrap();
    assert_eq!((inside.distance, inside.tile), (0.0, cell(1, 1, 2)));
    let near = nearest(&world, Vec2::new(124.0, 120.0), 500.0, &all()).unwrap();
    assert_eq!(near.tile, cell(1, 1, 2));
    let body = cast_ray(&world, Vec2::new(300.0, 0.0), Vec2::Y, 500.0, &all()).unwrap();
    assert_eq!((body.entity, body.tile), (Some(post), None));
    // An edit moves the hit to the new cell at the next step.
    world
        .resource_mut::<TilemapRegistry>()
        .set_tile("cells", low, 1, 1, solid);
    physics_step(&mut world);
    let down = cast_ray(&world, Vec2::new(124.0, 0.0), Vec2::Y, 500.0, &all()).unwrap();
    assert_eq!(
        (down.point, down.tile),
        (Vec2::new(124.0, 66.0), cell(1, 1, 1))
    );
}

/// A map of seeded size, tile size and origin: a render layer, then one or
/// two collision layers with about 40 % of their cells solid, the second in
/// physics layer 1.
fn seeded_map(rng: &mut crate::Pcg32) -> (TilemapData, Vec2) {
    let tile = [8, 16, 24][rng.next_u32() as usize % 3];
    let (width, height) = (4 + rng.next_u32() % 17, 3 + rng.next_u32() % 12);
    let mut data = TilemapData::new(tile, tile, width, height);
    let solid = data.add_tile("solid");
    let decor = data.add_layer("decor", LayerKind::Render);
    data.fill(decor, solid);
    let layers = 1 + rng.next_u32() % 2;
    for index in 0..layers {
        let layer = data.add_layer(format!("solid{index}"), LayerKind::Collision);
        for row in 0..height {
            for col in 0..width {
                if rng.next_f32_unit() < 0.4 {
                    data.set_tile(layer, col, row, solid);
                }
            }
        }
        if index == 1 {
            data.set_layer_physics(
                layer,
                crate::assets::LayerPhysics {
                    layers: 1 << 1,
                    mask: RigidBody::ALL_LAYERS,
                },
            );
        }
    }
    let origin = Vec2::new(rng.next_range(-100.0, 100.0), rng.next_range(-100.0, 100.0)).round();
    (data, origin)
}

#[test]
fn overlap_and_nearest_read_tiles_from_the_grid_as_before() {
    // Against a test of every solid cell's box, in tile order, as the index
    // held them: the same overlaps in the same order, and the same nearest
    // hit, its distance, point, normal and cell, ties to the earlier cell.
    let filters = [
        all(),
        all().with_mask(1),
        all().with_mask(1 << 1),
        all().without_layers(1 << 1),
        all().with_tiles(false),
    ];
    for seed in 0..200 {
        let mut rng = crate::Pcg32::seeded(0x5E9D + seed);
        let (data, origin) = seeded_map(&mut rng);
        let mut world = world();
        world
            .resource_mut::<TilemapRegistry>()
            .insert("seeded".into(), data.clone());
        let map = world.spawn();
        world.insert(map, TilemapInstance::new("seeded", origin));
        physics_step(&mut world);
        let (tw, th) = (data.tile_width as f32, data.tile_height as f32);
        let half = Vec2::new(tw * 0.5, th * 0.5);
        let mut cells = Vec::new();
        for (index, layer) in data.layers.iter().enumerate() {
            if layer.kind != LayerKind::Collision {
                continue;
            }
            let physics = data.layer_physics(index);
            for row in 0..data.height {
                for col in 0..data.width {
                    if data.tile_at(index, col, row).is_some_and(|tile| tile >= 0) {
                        let center = Vec2::new(
                            origin.x + (col as f32) * tw + half.x,
                            origin.y + (row as f32) * th + half.y,
                        );
                        let entry = IndexEntry {
                            entity: None,
                            center,
                            shape: Shape::Aabb { half_extents: half },
                            face_mask: FACE_ALL,
                            sensor: false,
                            identity: 0,
                            layers: physics.layers,
                            mask: physics.mask,
                        };
                        cells.push((
                            entry,
                            TileHit {
                                map,
                                layer: index,
                                col,
                                row,
                            },
                        ));
                    }
                }
            }
        }
        let size = data.pixel_size();
        let mut out = Vec::new();
        for query in 0..12 {
            let filter = filters[query % filters.len()];
            let place = |rng: &mut crate::Pcg32| {
                origin
                    + Vec2::new(
                        rng.next_range(-30.0, size.x + 30.0),
                        rng.next_range(-30.0, size.y + 30.0),
                    )
            };
            let shape = if rng.next_u32().is_multiple_of(2) {
                Shape::Circle {
                    radius: rng.next_range(0.0, 30.0),
                }
            } else {
                Shape::Aabb {
                    half_extents: Vec2::new(rng.next_range(0.0, 30.0), rng.next_range(0.0, 30.0)),
                }
            };
            let center = place(&mut rng);
            overlap(&world, shape, center, &filter, &mut out);
            let want: Vec<Option<Entity>> = cells
                .iter()
                .filter(|(entry, _)| {
                    filter.admits(entry, &world)
                        && shapes_overlap(shape, center, entry.shape, entry.center)
                })
                .map(|_| None)
                .collect();
            assert_eq!(out, want, "seed {seed}, query {query}: overlap");
            // Every other point on the cells' corners and edges, where
            // cells tie.
            let mut point = place(&mut rng);
            if query % 2 == 1 {
                point = origin + ((point - origin) / half).round() * half;
            }
            let reach = rng.next_range(0.0, 60.0);
            let mut best: Option<(SpatialHit, TileHit)> = None;
            for (entry, cell) in &cells {
                let hit = surface_hit(point, entry);
                if filter.admits(entry, &world)
                    && hit.distance <= reach
                    && best.is_none_or(|(kept, _)| hit.distance < kept.distance)
                {
                    best = Some((hit, *cell));
                }
            }
            let got = nearest(&world, point, reach, &filter)
                .map(|hit| (hit.distance, hit.point, hit.normal, hit.tile));
            let want = best.map(|(hit, cell)| (hit.distance, hit.point, hit.normal, Some(cell)));
            assert_eq!(got, want, "seed {seed}, query {query}: nearest");
        }
    }
}

/// A seeded map for the cast oracle: a render layer, then two collision
/// layers of 16 or 64 px cells, `width` across and `height` down, filling
/// toward the bottom in three materials, none one-way.
fn materials_map(rng: &mut crate::Pcg32, width: u32, height: u32) -> TilemapData {
    let tile = if rng.next_u32().is_multiple_of(2) {
        16
    } else {
        64
    };
    let mut data = TilemapData::new(tile, tile, width, height);
    let plain = data.add_tile("plain");
    for material in ["sand", "stone"] {
        let tile = data.add_tile(material);
        data.set_tile_props(
            tile,
            crate::assets::TileProps {
                material: Some(material.into()),
                ..crate::assets::TileProps::default()
            },
        );
    }
    let decor = data.add_layer("decor", LayerKind::Render);
    data.fill(decor, plain);
    for index in 0..2 {
        let layer = data.add_layer(format!("solid{index}"), LayerKind::Collision);
        for row in 0..height {
            for col in 0..width {
                // Rows fill more toward the bottom, so floors and walls form.
                if rng.next_f32_unit() < 0.25 + 0.5 * (row as f32 / height as f32) {
                    let tile = [plain, plain, 1, 2][rng.next_u32() as usize % 4];
                    data.set_tile(layer, col, row, tile);
                }
            }
        }
    }
    data
}

/// Each solid cell of `world`'s maps as today's index held it, in tile
/// order: its box with the face mask its solid neighbours leave, and its
/// cell.
fn todays_cells(world: &World) -> Vec<(IndexEntry, TileHit)> {
    let registry = world.resource::<TilemapRegistry>();
    let mut cells = Vec::new();
    for (map, instance) in world.query::<(Entity, &TilemapInstance)>() {
        let data = registry.get(&instance.id).unwrap();
        let (tw, th) = (data.tile_width as f32, data.tile_height as f32);
        let half = Vec2::new(tw * 0.5, th * 0.5);
        for (index, layer) in data.layers.iter().enumerate() {
            if layer.kind != LayerKind::Collision {
                continue;
            }
            let physics = data.layer_physics(index);
            let solid = |col: i64, row: i64| {
                col >= 0
                    && row >= 0
                    && data
                        .tile_at(index, col as u32, row as u32)
                        .is_some_and(|tile| tile >= 0)
            };
            for row in 0..data.height {
                for col in 0..data.width {
                    let (c, r) = (i64::from(col), i64::from(row));
                    if !solid(c, r) {
                        continue;
                    }
                    let mut face_mask = 0;
                    for (face, (dc, dr)) in [
                        (FACE_TOP, (0, -1)),
                        (FACE_BOTTOM, (0, 1)),
                        (FACE_LEFT, (-1, 0)),
                        (FACE_RIGHT, (1, 0)),
                    ] {
                        if !solid(c + dc, r + dr) {
                            face_mask |= face;
                        }
                    }
                    let entry = IndexEntry {
                        entity: None,
                        center: Vec2::new(
                            instance.origin.x + (col as f32) * tw + half.x,
                            instance.origin.y + (row as f32) * th + half.y,
                        ),
                        shape: Shape::Aabb { half_extents: half },
                        face_mask,
                        sensor: false,
                        identity: 0,
                        layers: physics.layers,
                        mask: physics.mask,
                    };
                    let cell = TileHit {
                        map,
                        layer: index,
                        col,
                        row,
                    };
                    cells.push((entry, cell));
                }
            }
        }
    }
    cells
}

#[test]
fn casts_on_run_boxes_give_the_cells_hits() {
    // Rays, and boxes and circles 0.1 to 2.5 cells across, from anywhere
    // over two overlapping maps, inside the terrain or out: the same first
    // hit, its distance, normal, point and cell, as each solid cell with the
    // face mask its neighbours leave gives, ties to the earlier cell.
    for seed in 0..200 {
        let mut rng = crate::Pcg32::seeded(0x0CA5_7000 + seed);
        let mut world = world();
        let (width, height) = (8 + rng.next_u32() % 33, 6 + rng.next_u32() % 25);
        let a = materials_map(&mut rng, width, height);
        let tile = Vec2::new(a.tile_width as f32, a.tile_height as f32);
        let size = a.pixel_size();
        let origin =
            Vec2::new(rng.next_range(-200.0, 200.0), rng.next_range(-200.0, 200.0)).round();
        // A smaller map over part of the first, so maps tie too.
        let (b_width, b_height) = (4 + rng.next_u32() % 10, 3 + rng.next_u32() % 8);
        let b = materials_map(&mut rng, b_width, b_height);
        let shift = (Vec2::new(rng.next_range(0.0, size.x), rng.next_range(0.0, size.y)) / tile)
            .round()
            * tile;
        for (id, data, at) in [("a", a, origin), ("b", b, origin + shift)] {
            world
                .resource_mut::<TilemapRegistry>()
                .insert(id.into(), data);
            let map = world.spawn();
            world.insert(map, TilemapInstance::new(id, at));
        }
        physics_step(&mut world);
        let cells = todays_cells(&world);
        for probe in 0..30 {
            let from = origin
                + Vec2::new(
                    rng.next_range(-2.0 * tile.x, size.x + 2.0 * tile.x),
                    rng.next_range(-2.0 * tile.y, size.y + 2.0 * tile.y),
                );
            let dir = rng.next_unit_vec2();
            let reach = rng.next_range(0.0, size.max_element() * 1.5);
            let across = rng.next_range(0.1, 2.5) * tile.x;
            let shape = match probe % 3 {
                0 => None,
                1 => Some(Shape::Aabb {
                    half_extents: Vec2::new(across, rng.next_range(0.1, 2.5) * tile.y) * 0.5,
                }),
                _ => Some(Shape::Circle {
                    radius: across * 0.5,
                }),
            };
            // The direction as the casts normalize it.
            let unit = dir.try_normalize().unwrap();
            let mut want: Option<SpatialHit> = None;
            for (entry, cell) in &cells {
                let hit = match shape {
                    None => ray_against(from, unit, entry).map(|(distance, normal)| SpatialHit {
                        entity: None,
                        point: from + unit * distance,
                        normal,
                        distance,
                        layers: entry.layers,
                        tile: None,
                    }),
                    Some(shape) => cast_against(shape, from, unit, entry),
                };
                if let Some(hit) = hit
                    && hit.distance <= reach
                    && want.is_none_or(|kept| hit.distance < kept.distance)
                {
                    want = Some(SpatialHit {
                        tile: Some(*cell),
                        ..hit
                    });
                }
            }
            let got = match shape {
                None => cast_ray(&world, from, dir, reach, &all()),
                Some(shape) => cast_shape(&world, shape, from, dir, reach, &all()),
            };
            // As bits, so a normal's -0.0 is not its 0.0.
            let read = |hit: SpatialHit| {
                (
                    hit.distance.to_bits(),
                    hit.normal.to_array().map(f32::to_bits),
                    hit.point.to_array().map(f32::to_bits),
                    hit.tile,
                    hit.layers,
                )
            };
            assert_eq!(
                got.map(read),
                want.map(read),
                "seed {seed}, probe {probe}: {shape:?} from {from} along {dir} for {reach}"
            );
        }
    }
}
