use super::*;

fn aabb(cx: f32, cy: f32, hx: f32, hy: f32) -> Aabb {
    Aabb::new(Vec2::new(cx, cy), Vec2::new(hx, hy))
}

/// Layout of the last build; query once before asking.
fn is_direct(grid: &SpatialGrid) -> bool {
    matches!(grid.layout, Layout::Direct { .. })
}

/// Stage one point far from everything else, which makes the staged bounds
/// too sparse for a direct table. No query near the origin returns it.
const FAR_ID: ProxyId = 9_999;
fn force_hashed(grid: &mut SpatialGrid) {
    grid.insert(FAR_ID, &aabb(4.0e6, -4.0e6, 0.0, 0.0));
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
    assert!(
        !is_direct(&grid),
        "far-apart cells must use the hashed table"
    );
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

#[test]
fn floor_to_i32_equals_floor_then_cast_for_every_class_of_value() {
    let edges = [
        0.0,
        -0.0,
        0.5,
        -0.5,
        1.0,
        -1.0,
        1.5,
        -1.5,
        31.999_998,
        32.0,
        -32.0,
        -32.000_004,
        f32::EPSILON,
        -f32::EPSILON,
        f32::MIN_POSITIVE,
        -f32::MIN_POSITIVE,
        8_388_607.5,
        -8_388_607.5,
        16_777_216.0,
        -16_777_216.0,
        2_147_483_520.0,
        2_147_483_648.0,
        -2_147_483_648.0,
        -2_147_483_904.0,
        3.0e9,
        -3.0e9,
        1.0e12,
        -1.0e12,
        f32::MAX,
        f32::MIN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
    ];
    for x in edges {
        assert_eq!(floor_to_i32(x), x.floor() as i32, "{x:?}");
    }
    // Every 4,099th bit pattern: each exponent, both signs, NaN payloads.
    let mut bits = 0u32;
    loop {
        let x = f32::from_bits(bits);
        assert_eq!(floor_to_i32(x), x.floor() as i32, "bits {bits:#010x}");
        match bits.checked_add(4_099) {
            Some(next) => bits = next,
            None => break,
        }
    }
}

#[test]
fn cell_range_keeps_its_edge_rules() {
    let grid = SpatialGrid::new(32.0);
    let range = |cx, cy, hx, hy| grid.cell_range(&aabb(cx, cy, hx, hy));
    let cell = |x, y| (IVec2::new(x, y), IVec2::new(x, y));
    // A max edge exactly on a boundary does not claim the next cell.
    assert_eq!(range(16.0, 16.0, 16.0, 16.0), cell(0, 0));
    assert_eq!(range(-16.0, -16.0, 16.0, 16.0), cell(-1, -1));
    // A min edge on a boundary belongs to the cell it starts.
    assert_eq!(range(48.0, -48.0, 16.0, 16.0), cell(1, -2));
    // Negative non-integers floor away from zero.
    assert_eq!(range(-0.5, -33.0, 0.25, 0.25), cell(-1, -2));
    assert_eq!(
        range(0.0, 0.0, 40.0, 40.0),
        (IVec2::splat(-2), IVec2::splat(1))
    );
    // Coordinates beyond `i32` cells saturate.
    assert_eq!(range(1.0e12, -1.0e12, 4.0, 4.0), cell(i32::MAX, i32::MIN));
}

#[test]
fn direct_table_is_chosen_up_to_four_slots_per_ref() {
    let mut out = Vec::new();
    let point = |x: f32| aabb(x * 32.0 + 16.0, 16.0, 0.0, 0.0);
    // Two refs: 8 cells of bounds is the limit, 9 falls back.
    for (far_cell, direct) in [(7.0, true), (8.0, false)] {
        let mut grid = SpatialGrid::new(32.0);
        grid.insert(0, &point(0.0));
        grid.insert(1, &point(far_cell));
        grid.query(&point(far_cell), None, &mut out);
        assert_eq!(out, vec![1]);
        assert_eq!(is_direct(&grid), direct, "far cell {far_cell}");
    }

    // The layout follows the staged bounds from one build to the next.
    let mut grid = SpatialGrid::new(32.0);
    let block = |grid: &mut SpatialGrid| {
        for id in 0..64u32 {
            let (col, row) = ((id % 8) as f32, (id / 8) as f32);
            grid.insert(id, &aabb(col * 32.0 + 16.0, row * 32.0 + 16.0, 4.0, 4.0));
        }
    };
    block(&mut grid);
    grid.query(&aabb(16.0, 16.0, 4.0, 4.0), None, &mut out);
    assert_eq!(out, vec![0]);
    assert!(is_direct(&grid));
    force_hashed(&mut grid);
    grid.query(&aabb(16.0, 16.0, 4.0, 4.0), None, &mut out);
    assert_eq!(out, vec![0]);
    assert!(!is_direct(&grid));
    grid.query(&aabb(4.0e6, -4.0e6, 1.0, 1.0), None, &mut out);
    assert_eq!(out, vec![FAR_ID]);
    grid.clear();
    block(&mut grid);
    grid.query(&aabb(240.0, 240.0, 4.0, 4.0), None, &mut out);
    assert_eq!(out, vec![63]);
    assert!(is_direct(&grid));
    // Queries beside and beyond the direct table's bounds find nothing.
    grid.query(&aabb(-500.0, 16.0, 100.0, 4.0), None, &mut out);
    assert!(out.is_empty(), "got: {out:?}");
    grid.query(&aabb(1.0e12, -1.0e12, 4.0, 4.0), None, &mut out);
    assert!(out.is_empty(), "got: {out:?}");
}

#[test]
fn layouts_return_the_same_ids_in_the_same_order() {
    // D-080: the direct table must not change what a query returns or its
    // order. Both grids stage the same proxies; the far point only makes the
    // second one's bounds sparse.
    use crate::Pcg32;

    for seed in 0..6u64 {
        let mut rng = Pcg32::seeded(0xD080_0000 + seed);
        let mut direct = SpatialGrid::new([8.0, 32.0, 50.0][seed as usize % 3]);
        let mut hashed = direct.clone();
        // Seeds 4 and 5 stage points only: the no-dedupe single-cell path.
        let points_only = seed >= 4;
        for id in 0..400u32 {
            let half = match id % 4 {
                _ if points_only => Vec2::ZERO,
                1 => Vec2::splat(rng.next_range(1.0, 6.0)),
                2 => Vec2::new(rng.next_range(1.0, 90.0), rng.next_range(1.0, 20.0)),
                3 => Vec2::splat(rng.next_range(10.0, 70.0)),
                _ => Vec2::ZERO,
            };
            let center = Vec2::new(rng.next_range(-300.0, 500.0), rng.next_range(-400.0, 200.0));
            let bounds = Aabb::new(center, half);
            direct.insert(id, &bounds);
            hashed.insert(id, &bounds);
        }
        force_hashed(&mut hashed);
        let mut from_direct = Vec::new();
        let mut from_hashed = Vec::new();
        let mut returned = 0;
        for probe_index in 0..300u32 {
            let probe = Aabb::new(
                Vec2::new(rng.next_range(-400.0, 600.0), rng.next_range(-500.0, 300.0)),
                Vec2::new(rng.next_range(0.0, 120.0), rng.next_range(0.0, 120.0)),
            );
            let exclude = (probe_index % 3 == 0).then_some(probe_index);
            direct.query(&probe, exclude, &mut from_direct);
            hashed.query(&probe, exclude, &mut from_hashed);
            assert_eq!(from_direct, from_hashed, "seed {seed}, {probe:?}");
            returned += from_direct.len();
        }
        assert!(is_direct(&direct) && !is_direct(&hashed), "seed {seed}");
        assert_eq!(direct.single_cell_ids, points_only, "seed {seed}");
        assert!(returned > 1_000, "seed {seed}: probes returned {returned}");
    }
}

#[test]
fn hashed_layout_keeps_the_basic_contracts() {
    // Small scenes build a direct table, so the tests above cover that
    // layout. These are their twins on the hashed fall-back.
    let mut out = Vec::new();

    // A shape spanning cells is returned once, on every query.
    let mut grid = SpatialGrid::new(32.0);
    let wide = aabb(0.0, 0.0, 40.0, 40.0);
    grid.insert(7, &wide);
    force_hashed(&mut grid);
    for _ in 0..2 {
        grid.query(&wide, None, &mut out);
        assert_eq!(out, vec![7]);
    }
    assert!(!is_direct(&grid));

    // Neighbours are candidates, the excluded id is not, and a later insert
    // rebuilds.
    let mut grid = SpatialGrid::new(32.0);
    grid.insert(0, &aabb(0.0, 0.0, 4.0, 4.0));
    grid.insert(1, &aabb(6.0, 0.0, 4.0, 4.0));
    force_hashed(&mut grid);
    grid.query(&aabb(0.0, 0.0, 4.0, 4.0), Some(0), &mut out);
    assert_eq!(out, vec![1]);
    grid.insert(2, &aabb(-6.0, 0.0, 4.0, 4.0));
    grid.query(&aabb(0.0, 0.0, 4.0, 4.0), Some(0), &mut out);
    out.sort_unstable();
    assert_eq!(out, vec![1, 2]);
    assert!(!is_direct(&grid));

    // A repeated id is still returned once.
    let mut grid = SpatialGrid::new(32.0);
    grid.insert(3, &aabb(10.0, 10.0, 0.0, 0.0));
    grid.insert(3, &aabb(12.0, 12.0, 0.0, 0.0));
    grid.insert(4, &aabb(50.0, 10.0, 0.0, 0.0));
    force_hashed(&mut grid);
    grid.query(&aabb(30.0, 10.0, 40.0, 10.0), None, &mut out);
    assert_eq!(out, vec![3, 4]);
    assert!(!is_direct(&grid));

    // `clear` drops every entry.
    grid.clear();
    grid.insert(5, &aabb(200.0, 200.0, 4.0, 4.0));
    force_hashed(&mut grid);
    grid.query(&aabb(30.0, 10.0, 40.0, 10.0), None, &mut out);
    assert!(out.is_empty(), "stale entry survived clear: {out:?}");
    grid.query(&aabb(200.0, 200.0, 4.0, 4.0), None, &mut out);
    assert_eq!(out, vec![5]);
    assert!(!is_direct(&grid));
}

#[test]
fn proxy_beyond_i32_cells_is_still_found() {
    let mut grid = SpatialGrid::new(32.0);
    grid.insert(0, &aabb(0.0, 0.0, 4.0, 4.0));
    grid.insert(1, &aabb(1.0e12, -1.0e12, 4.0, 4.0));
    let mut out = Vec::new();
    grid.query(&aabb(1.0e12, -1.0e12, 4.0, 4.0), None, &mut out);
    assert_eq!(out, vec![1]);
    grid.query(&aabb(0.0, 0.0, 4.0, 4.0), None, &mut out);
    assert_eq!(out, vec![0]);
}
