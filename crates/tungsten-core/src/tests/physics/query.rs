use super::*;
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
