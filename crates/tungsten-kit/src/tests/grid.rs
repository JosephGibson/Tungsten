use super::*;
use tungsten_core::physics::{Position, PrevPosition};
use tungsten_core::{Transform, World};

use crate::damage::health::Health;
use crate::overlap::hit::{HitShape, Hurtbox};
use crate::transform::hierarchy::spawn_child;

fn circle(radius: f32, start: Vec2, end: Vec2) -> Sweep {
    Sweep {
        shape: Shape::Circle { radius },
        start,
        end,
    }
}

fn aabb(half: Vec2, start: Vec2, end: Vec2) -> Sweep {
    Sweep {
        shape: Shape::Aabb { half_extents: half },
        start,
        end,
    }
}

fn still(sweep: Sweep) -> Sweep {
    Sweep {
        end: sweep.start,
        ..sweep
    }
}

fn close(a: Option<f32>, b: f32) -> bool {
    a.is_some_and(|a| (a - b).abs() < 1e-5)
}

#[test]
fn each_shape_pair_meets_at_its_first_contact() {
    let mover = |shape: Shape| Sweep {
        shape,
        start: Vec2::ZERO,
        end: Vec2::new(10.0, 0.0),
    };
    let at = Vec2::new(8.0, 0.0);
    let small_circle = Shape::Circle { radius: 1.0 };
    let small_box = Shape::Aabb {
        half_extents: Vec2::ONE,
    };
    // Circle and circle: |-8 + 10t| < 1 + 3.
    let t = first_contact(&mover(small_circle), &still(circle(3.0, at, at)));
    assert!(close(t, 0.4), "{t:?}");
    // Box and box: |-8 + 10t| < 1 + 2.
    let t = first_contact(&mover(small_box), &still(aabb(Vec2::splat(2.0), at, at)));
    assert!(close(t, 0.5), "{t:?}");
    // A circle on a box's face, and a box on a circle.
    let t = first_contact(&mover(small_circle), &still(aabb(Vec2::splat(2.0), at, at)));
    assert!(close(t, 0.5), "{t:?}");
    let t = first_contact(&mover(small_box), &still(circle(2.0, at, at)));
    assert!(close(t, 0.5), "{t:?}");
    // A circle on a box's corner, along the diagonal: the round corner.
    let diagonal = circle(1.0, Vec2::splat(-10.0), Vec2::ZERO);
    let t = first_contact(
        &diagonal,
        &still(aabb(Vec2::splat(2.0), Vec2::ZERO, Vec2::ZERO)),
    );
    let corner = (8.0 - std::f32::consts::FRAC_1_SQRT_2) / 10.0;
    assert!(close(t, corner), "{t:?} against {corner}");
}

#[test]
fn shapes_that_start_overlapping_meet_at_zero() {
    let a = circle(2.0, Vec2::ZERO, Vec2::new(5.0, 0.0));
    let b = still(aabb(Vec2::splat(2.0), Vec2::new(1.0, 1.0), Vec2::ZERO));
    assert_eq!(first_contact(&a, &b), Some(0.0));
    let resting = still(circle(1.0, Vec2::ZERO, Vec2::ZERO));
    assert_eq!(
        first_contact(&resting, &resting),
        Some(0.0),
        "no motion, overlapping"
    );
    let apart = still(circle(1.0, Vec2::new(5.0, 0.0), Vec2::ZERO));
    assert_eq!(first_contact(&resting, &apart), None, "no motion, apart");
}

#[test]
fn a_graze_that_never_overlaps_misses() {
    let target = still(circle(1.0, Vec2::ZERO, Vec2::ZERO));
    let tangent = circle(1.0, Vec2::new(-10.0, 2.0), Vec2::new(10.0, 2.0));
    assert_eq!(
        first_contact(&tangent, &target),
        None,
        "a circle grazing a circle"
    );
    let block = still(aabb(Vec2::ONE, Vec2::ZERO, Vec2::ZERO));
    let slide = aabb(Vec2::ONE, Vec2::new(-10.0, 2.0), Vec2::new(10.0, 2.0));
    assert_eq!(
        first_contact(&slide, &block),
        None,
        "a box sliding along a face"
    );
    let corner = circle(1.0, Vec2::new(-10.0, 2.0), Vec2::new(10.0, 2.0));
    assert_eq!(
        first_contact(&corner, &block),
        None,
        "a circle along a face, touching"
    );
}

#[test]
fn two_paths_that_cross_inside_a_step_meet_once_their_ends_apart() {
    let bolt = circle(1.0, Vec2::new(-10.0, 0.0), Vec2::new(10.0, 0.0));
    let runner = circle(1.0, Vec2::new(0.0, -10.0), Vec2::new(0.0, 10.0));
    let t = first_contact(&bolt, &runner);
    let expected = (10.0 - 2.0 / std::f32::consts::SQRT_2) / 20.0;
    assert!(close(t, expected), "{t:?} against {expected}");
    assert!(bolt.start.distance(runner.start) > 2.0 && bolt.end.distance(runner.end) > 2.0);
}

#[test]
fn a_fast_pass_through_a_thin_target_meets_inside_the_step() {
    let bolt = circle(1.0, Vec2::new(-30.0, 0.0), Vec2::new(30.0, 0.0));
    let wafer = still(aabb(Vec2::new(0.5, 4.0), Vec2::ZERO, Vec2::ZERO));
    let t = first_contact(&bolt, &wafer);
    assert!(close(t, (30.0 - 1.5) / 60.0), "{t:?}");
}

#[test]
fn a_contact_at_or_after_the_steps_end_is_none() {
    let target = still(circle(1.0, Vec2::new(12.0, 0.0), Vec2::ZERO));
    let short = circle(1.0, Vec2::ZERO, Vec2::new(10.0, 0.0));
    assert_eq!(
        first_contact(&short, &target),
        None,
        "touches at t = 1 exactly"
    );
    let shorter = circle(1.0, Vec2::ZERO, Vec2::new(5.0, 0.0));
    assert_eq!(first_contact(&shorter, &target), None);
    let behind = circle(1.0, Vec2::new(20.0, 0.0), Vec2::new(30.0, 0.0));
    assert_eq!(first_contact(&behind, &target), None, "moving away");
}

/// A World of hurtboxes: a circle of `radius` at each point.
fn hurtboxes(points: &[(f32, f32)], radius: f32) -> (World, Vec<Entity>) {
    let mut world = World::new();
    let entities = points
        .iter()
        .map(|&(x, y)| {
            world.spawn_with((
                Transform::from_position(Vec2::new(x, y)),
                Hurtbox::new(HitShape::circle(radius)),
            ))
        })
        .collect();
    (world, entities)
}

#[test]
fn candidates_come_in_entry_order_each_once() {
    let (world, entities) = hurtboxes(
        &[(500.0, 0.0), (10.0, 10.0), (60.0, 60.0), (70.0, 0.0)],
        30.0,
    );
    let mut grid = HitGrid::default();
    grid.rebuild(&world);
    let ids: Vec<Entity> = grid.entries().iter().map(|entry| entry.entity).collect();
    assert_eq!(ids, entities, "by index");
    let mut out = Vec::new();
    grid.candidates(Vec2::new(-10.0, -10.0), Vec2::new(100.0, 100.0), &mut out);
    assert_eq!(out, [1, 2, 3], "each once, though each fills several cells");
    grid.candidates(
        Vec2::new(2000.0, 2000.0),
        Vec2::new(2100.0, 2100.0),
        &mut out,
    );
    assert!(out.is_empty(), "outside the occupied bounds");
}

#[test]
fn an_entry_past_64_cells_is_met_through_the_oversized_list() {
    let (mut world, entities) = hurtboxes(&[(0.0, 0.0), (5000.0, 5000.0)], 4.0);
    world.insert(entities[0], Hurtbox::new(HitShape::circle(1000.0)));
    let mut grid = HitGrid::default();
    grid.rebuild(&world);
    assert_eq!(grid.oversized, [0], "2000 px across fills 32 by 32 cells");
    let mut out = Vec::new();
    grid.candidates(Vec2::new(900.0, -5.0), Vec2::new(910.0, 5.0), &mut out);
    assert_eq!(out, [0]);
    let near = circle(2.0, Vec2::new(905.0, 0.0), Vec2::new(905.0, 0.0));
    assert_eq!(first_contact(&near, &grid.entries()[0].sweep), Some(0.0));
}

#[test]
fn a_sweep_past_the_occupied_bounds_tests_every_entry() {
    let points: Vec<(f32, f32)> = (0..10)
        .map(|i| (i as f32 * 1000.0, i as f32 * 1000.0))
        .collect();
    let (world, _) = hurtboxes(&points, 4.0);
    let mut grid = HitGrid::default();
    grid.rebuild(&world);
    let (lo, hi) = grid.bounds.unwrap();
    let (_, _, cells) = cell_span(lo, hi);
    assert!(
        cells > 64,
        "the clamped sweep spans {cells} cells: every entry, no cell walk"
    );
    let mut out = Vec::new();
    grid.candidates(Vec2::new(-1e6, -1e6), Vec2::new(1e6, 1e6), &mut out);
    assert_eq!(out, (0..10).collect::<Vec<u32>>());
    let bolt = circle(1.0, Vec2::splat(-1e5), Vec2::splat(1e5));
    let met: Vec<bool> = out
        .iter()
        .map(|&i| first_contact(&bolt, &grid.entries()[i as usize].sweep).is_some())
        .collect();
    assert!(met.iter().all(|&m| m), "the fast pass meets each");
}

#[test]
fn the_grid_leaves_out_the_dead_the_despawning_and_the_unplaced() {
    let (mut world, entities) = hurtboxes(&[(0.0, 0.0), (10.0, 0.0), (20.0, 0.0)], 4.0);
    world.insert(entities[0], Health::new(1.0));
    crate::damage(&mut world, entities[0], 1.0);
    world.insert(entities[1], Despawning);
    world.spawn_with((Hurtbox::new(HitShape::circle(4.0)),));
    let mut grid = HitGrid::default();
    grid.rebuild(&world);
    let ids: Vec<Entity> = grid.entries().iter().map(|entry| entry.entity).collect();
    assert_eq!(ids, [entities[2]]);
}

#[test]
fn a_root_sweeps_from_its_history_and_a_child_stands_at_its_transform() {
    let mut world = World::new();
    let root = world.spawn_with((
        Position::new(30.0, 0.0),
        PrevPosition(Vec2::new(10.0, 0.0)),
        Transform::from_position(Vec2::new(25.0, 0.0)),
    ));
    let bare = world.spawn_with((Position::new(5.0, 5.0),));
    let child = spawn_child(
        &mut world,
        root,
        Vec2::new(0.0, 8.0),
        (Position::new(99.0, 99.0),),
    );
    assert_eq!(
        sweep_points(&world, root),
        Some((Vec2::new(10.0, 0.0), Vec2::new(30.0, 0.0)))
    );
    assert_eq!(
        sweep_points(&world, bare),
        Some((Vec2::splat(5.0), Vec2::splat(5.0)))
    );
    let drawn = world.get::<Transform>(child).unwrap().position;
    assert_eq!(
        sweep_points(&world, child),
        Some((drawn, drawn)),
        "its last propagation"
    );
    let shape = Shape::Circle { radius: 1.0 };
    let offset = Vec2::new(2.0, 0.0);
    let sweep = sweep_of(&world, root, shape, offset).unwrap();
    assert_eq!(
        (sweep.start, sweep.end),
        (Vec2::new(12.0, 0.0), Vec2::new(32.0, 0.0))
    );
    assert_eq!(sweep.at(0.5), Vec2::new(22.0, 0.0));
    assert_eq!(
        sweep.bounds(),
        (Vec2::new(11.0, -1.0), Vec2::new(33.0, 1.0))
    );
}

fn close_normal(found: Option<(f32, Vec2)>, t: f32, normal: Vec2) -> bool {
    found.is_some_and(|(at, n)| (at - t).abs() < 1e-5 && (n - normal).length() < 1e-5)
}

#[test]
fn a_wall_contact_keeps_the_face_normal_for_each_shape_pair() {
    let wall_box = still(aabb(Vec2::splat(2.0), Vec2::new(8.0, 0.0), Vec2::ZERO));
    let wall_circle = still(circle(3.0, Vec2::new(8.0, 0.0), Vec2::ZERO));
    let right = |shape: Shape| Sweep {
        shape,
        start: Vec2::ZERO,
        end: Vec2::new(10.0, 0.0),
    };
    let ball = Shape::Circle { radius: 1.0 };
    let block = Shape::Aabb {
        half_extents: Vec2::ONE,
    };
    // Into a box's left face, a circle or a box.
    assert!(close_normal(
        wall_contact(&right(ball), &wall_box),
        0.5,
        Vec2::NEG_X
    ));
    assert!(close_normal(
        wall_contact(&right(block), &wall_box),
        0.5,
        Vec2::NEG_X
    ));
    // Onto a circle, along the line of centres.
    assert!(close_normal(
        wall_contact(&right(ball), &wall_circle),
        0.4,
        Vec2::NEG_X
    ));
    assert!(close_normal(
        wall_contact(&right(block), &wall_circle),
        0.4,
        Vec2::NEG_X
    ));
    // Down onto a box's top face.
    let falling = circle(1.0, Vec2::new(8.0, -10.0), Vec2::new(8.0, 0.0));
    assert!(close_normal(
        wall_contact(&falling, &wall_box),
        0.7,
        Vec2::NEG_Y
    ));
    // On a box's corner, along the diagonal: the corner's normal.
    let diagonal = circle(1.0, Vec2::splat(-10.0), Vec2::ZERO);
    let corner = still(aabb(Vec2::splat(2.0), Vec2::ZERO, Vec2::ZERO));
    let (_, normal) = wall_contact(&diagonal, &corner).unwrap();
    assert!(
        (normal - Vec2::splat(-std::f32::consts::FRAC_1_SQRT_2)).length() < 1e-5,
        "{normal}"
    );
}

#[test]
fn a_shape_inside_a_wall_meets_it_at_once_moving_in_and_leaves_it_moving_out() {
    let wall = still(aabb(Vec2::new(10.0, 10.0), Vec2::ZERO, Vec2::ZERO));
    // Inside near the left face: pushed out to the left.
    let inward = circle(1.0, Vec2::new(-9.5, 0.0), Vec2::new(0.0, 0.0));
    assert!(close_normal(wall_contact(&inward, &wall), 0.0, Vec2::NEG_X));
    let outward = circle(1.0, Vec2::new(-9.5, 0.0), Vec2::new(-20.0, 0.0));
    assert_eq!(wall_contact(&outward, &wall), None, "it can always leave");
    // Resting against a face, it slides along it.
    let along = circle(1.0, Vec2::new(-11.0, 0.0), Vec2::new(-11.0, 5.0));
    assert_eq!(wall_contact(&along, &wall), None);
}

#[test]
fn a_velocity_reflects_about_a_normal() {
    assert_eq!(reflect(Vec2::new(3.0, -4.0), Vec2::Y), Vec2::new(3.0, 4.0));
    assert_eq!(
        reflect(Vec2::new(5.0, 1.0), Vec2::NEG_X),
        Vec2::new(-5.0, 1.0)
    );
    let diagonal = Vec2::splat(std::f32::consts::FRAC_1_SQRT_2);
    let back = reflect(Vec2::new(-1.0, -1.0), diagonal);
    assert!((back - Vec2::ONE).length() < 1e-5, "{back}");
}

#[test]
fn strike_types_the_untyped_parts_with_the_items_tags() {
    use tungsten_core::{Plugin, Schedule, Stage, Time};

    let mut world = World::new();
    world.insert_resource(Time::new());
    let mut schedule = Schedule::new();
    crate::TagsPlugin::new()
        .with_tags(["fire", "ice"])
        .build(&mut schedule, &mut world);
    crate::HealthPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    let types: crate::damage::kinds::DamageTypes =
        serde_json::from_value(serde_json::json!({ "fire": {}, "ice": {} })).unwrap();
    world
        .resource_mut::<tungsten_core::DataRegistry>()
        .insert("game", types);
    let target = world.spawn_with((Health::new(100.0),));
    let hitter = world.spawn();
    let typed = HitPayload {
        parts: vec![
            crate::DamagePart::new(2.0),
            crate::DamagePart::new(3.0).with_tags(Tags::new(["ice"])),
        ],
        penetration: None,
        conversion: None,
    };
    let fire = Tags::new(["fire"]);
    strike(
        &mut world,
        &Blow {
            hitter,
            target,
            hitter_at: Vec2::ZERO,
            target_at: Vec2::new(1.0, 0.0),
            relative: Vec2::ZERO,
            damage: 5.0,
            tags: &fire,
            typed: Some(&typed),
            knockback: 0.0,
        },
    );
    world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
    schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
    let parts: Vec<(f32, Vec<&str>)> = world
        .resource::<EventQueue<crate::DamagedPart>>()
        .iter()
        .map(|part| (part.amount, part.kinds.names(&world)))
        .collect();
    assert_eq!(
        parts,
        [(2.0, vec!["fire"]), (3.0, vec!["ice"])],
        "the untyped part of the item's tags, the typed one of its own"
    );
    let hit = world
        .resource::<EventQueue<crate::Damaged>>()
        .iter()
        .next()
        .copied()
        .unwrap();
    assert_eq!((hit.amount, hit.source), (5.0, Some(hitter)));
}
