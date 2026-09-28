use super::*;

fn aabb(cx: f32, cy: f32, hx: f32, hy: f32) -> Aabb {
    Aabb::new(Vec2::new(cx, cy), Vec2::new(hx, hy))
}

#[test]
fn single_cell_insert_and_query() {
    let mut grid = SpatialGrid::new(32.0);
    let a = aabb(16.0, 16.0, 4.0, 4.0);
    grid.insert(0, &a);
    let mut out = Vec::new();
    grid.query(&a, None, &mut out);
    assert_eq!(out, vec![0]);
}

#[test]
fn shape_spanning_cells_is_returned_once() {
    let mut grid = SpatialGrid::new(32.0);
    let a = aabb(0.0, 0.0, 40.0, 40.0);
    grid.insert(7, &a);
    let mut out = Vec::new();
    grid.query(&a, None, &mut out);
    assert_eq!(out, vec![7]);
}

#[test]
fn separated_shapes_dont_collide() {
    let mut grid = SpatialGrid::new(32.0);
    grid.insert(0, &aabb(16.0, 16.0, 4.0, 4.0));
    grid.insert(1, &aabb(200.0, 200.0, 4.0, 4.0));
    let mut out = Vec::new();
    grid.query(&aabb(16.0, 16.0, 4.0, 4.0), Some(0), &mut out);
    assert!(out.is_empty(), "got: {out:?}");
}

#[test]
fn neighbours_are_candidates() {
    let mut grid = SpatialGrid::new(32.0);
    grid.insert(0, &aabb(0.0, 0.0, 4.0, 4.0));
    grid.insert(1, &aabb(6.0, 0.0, 4.0, 4.0));
    let mut out = Vec::new();
    grid.query(&aabb(0.0, 0.0, 4.0, 4.0), Some(0), &mut out);
    assert_eq!(out, vec![1]);
}

#[test]
fn repeated_queries_reset_dedup_marks() {
    let mut grid = SpatialGrid::new(32.0);
    let a = aabb(0.0, 0.0, 40.0, 40.0);
    grid.insert(7, &a);

    let mut out = Vec::new();
    grid.query(&a, None, &mut out);
    assert_eq!(out, vec![7]);

    grid.query(&a, None, &mut out);
    assert_eq!(out, vec![7]);
}

#[test]
fn clear_empties_grid() {
    let mut grid = SpatialGrid::new(32.0);
    grid.insert(0, &aabb(0.0, 0.0, 4.0, 4.0));
    grid.clear();
    let mut out = Vec::new();
    grid.query(&aabb(0.0, 0.0, 4.0, 4.0), None, &mut out);
    assert!(out.is_empty());
}

#[test]
fn negative_coords_work() {
    let mut grid = SpatialGrid::new(32.0);
    grid.insert(0, &aabb(-100.0, -100.0, 4.0, 4.0));
    let mut out = Vec::new();
    grid.query(&aabb(-100.0, -100.0, 4.0, 4.0), None, &mut out);
    assert_eq!(out, vec![0]);
}

#[test]
fn insert_after_query_rebuilds() {
    let mut grid = SpatialGrid::new(32.0);
    let a = aabb(16.0, 16.0, 4.0, 4.0);
    grid.insert(0, &a);
    let mut out = Vec::new();
    grid.query(&a, None, &mut out);
    assert_eq!(out, vec![0]);

    grid.insert(1, &aabb(20.0, 16.0, 4.0, 4.0));
    grid.query(&a, None, &mut out);
    out.sort_unstable();
    assert_eq!(out, vec![0, 1]);
}

#[test]
fn clear_then_reinsert_reuses_grid() {
    let mut grid = SpatialGrid::new(32.0);
    grid.insert(0, &aabb(16.0, 16.0, 4.0, 4.0));
    let mut out = Vec::new();
    grid.query(&aabb(16.0, 16.0, 4.0, 4.0), None, &mut out);
    assert_eq!(out, vec![0]);

    grid.clear();
    grid.insert(5, &aabb(200.0, 200.0, 4.0, 4.0));
    grid.query(&aabb(16.0, 16.0, 4.0, 4.0), None, &mut out);
    assert!(out.is_empty(), "stale entry survived clear: {out:?}");
    grid.query(&aabb(200.0, 200.0, 4.0, 4.0), None, &mut out);
    assert_eq!(out, vec![5]);
}

#[test]
fn hash_aliasing_never_produces_false_candidates() {
    // Slot table sized to ref count: many distinct far-apart cells guarantee
    // slot collisions; exact-cell compare must still keep queries precise.
    let mut grid = SpatialGrid::new(32.0);
    let mut positions = Vec::new();
    for i in 0..512u32 {
        let x = (i % 23) as f32 * 512.0 - 4_096.0;
        let y = (i / 23) as f32 * 512.0 - 4_096.0;
        positions.push(Vec2::new(x, y));
        grid.insert(i, &aabb(x, y, 4.0, 4.0));
    }
    let mut out = Vec::new();
    for (i, pos) in positions.iter().enumerate() {
        grid.query(&aabb(pos.x, pos.y, 4.0, 4.0), None, &mut out);
        assert_eq!(out, vec![i as u32], "at {pos:?}");
    }
}

#[test]
fn for_each_in_visits_query_results_in_order() {
    let mut grid = SpatialGrid::new(32.0);
    for id in 0..40u32 {
        let x = (id % 8) as f32 * 11.0 - 30.0;
        let y = (id / 8) as f32 * 13.0 - 20.0;
        let half = if id % 3 == 0 { 20.0 } else { 0.0 };
        grid.insert(id, &aabb(x, y, half, half));
    }
    let probe = aabb(5.0, 5.0, 30.0, 24.0);
    let mut queried = Vec::new();
    grid.query(&probe, Some(9), &mut queried);
    let mut visited = Vec::new();
    grid.for_each_in(&probe, Some(9), |id| visited.push(id));
    assert!(!queried.is_empty());
    assert!(!queried.contains(&9), "excluded id returned");
    assert_eq!(visited, queried);
}

#[test]
fn duplicate_point_ids_are_still_returned_once() {
    // A repeated id disables the single-cell no-dedupe path.
    let mut grid = SpatialGrid::new(32.0);
    grid.insert(3, &aabb(10.0, 10.0, 0.0, 0.0));
    grid.insert(3, &aabb(12.0, 12.0, 0.0, 0.0));
    grid.insert(4, &aabb(50.0, 10.0, 0.0, 0.0));
    let mut out = Vec::new();
    grid.query(&aabb(30.0, 10.0, 40.0, 10.0), None, &mut out);
    out.sort_unstable();
    assert_eq!(out, vec![3, 4]);
}

#[test]
fn point_grid_returns_each_covered_point_once() {
    let points: Vec<Vec2> = (0..300)
        .map(|i| {
            Vec2::new(
                ((i * 37) % 400) as f32 - 200.0,
                ((i * 53) % 300) as f32 - 150.0,
            )
        })
        .collect();
    let mut grid = SpatialGrid::new(32.0);
    for round in 0..2 {
        // Round two re-inserts the same ids after `clear`: still unique.
        grid.clear();
        for (id, &p) in points.iter().enumerate() {
            grid.insert(id as u32, &Aabb::new(p, Vec2::ZERO));
        }
        let probe = aabb(10.0, -5.0, 48.0, 40.0);
        let mut out = Vec::new();
        grid.query(&probe, Some(0), &mut out);
        let mut unique = out.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), out.len(), "round {round}: duplicate ids");
        for (id, &p) in points.iter().enumerate().skip(1) {
            let inside = p.cmpge(probe.min()).all() && p.cmplt(probe.max()).all();
            if inside {
                assert!(out.contains(&(id as u32)), "round {round}: missed {id}");
            }
        }
    }
}
