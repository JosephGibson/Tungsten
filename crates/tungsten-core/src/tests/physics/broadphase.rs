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
    // AABB spans 4 cells (straddles origin).
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
