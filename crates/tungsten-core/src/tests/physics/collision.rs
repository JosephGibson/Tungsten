use super::*;

fn aabb(cx: f32, cy: f32, hx: f32, hy: f32) -> Aabb {
    Aabb::new(Vec2::new(cx, cy), Vec2::new(hx, hy))
}

#[test]
fn aabb_separated_returns_none() {
    let a = aabb(0.0, 0.0, 1.0, 1.0);
    let b = aabb(5.0, 0.0, 1.0, 1.0);
    assert!(aabb_vs_aabb(&a, &b).is_none());
}

#[test]
fn aabb_touching_is_separated() {
    let a = aabb(0.0, 0.0, 1.0, 1.0);
    let b = aabb(2.0, 0.0, 1.0, 1.0);
    assert!(aabb_vs_aabb(&a, &b).is_none());
}

#[test]
fn aabb_overlap_x_axis_gives_x_normal() {
    let a = aabb(0.0, 0.0, 1.0, 1.0);
    let b = aabb(1.5, 0.0, 1.0, 1.0);
    let c = aabb_vs_aabb(&a, &b).unwrap();
    assert_eq!(c.normal, Vec2::new(-1.0, 0.0));
    assert!((c.penetration - 0.5).abs() < 1e-5);
}

#[test]
fn aabb_overlap_y_axis_gives_y_normal() {
    let a = aabb(0.0, 0.0, 2.0, 1.0);
    let b = aabb(0.0, 1.5, 2.0, 1.0);
    let c = aabb_vs_aabb(&a, &b).unwrap();
    assert_eq!(c.normal, Vec2::new(0.0, -1.0));
    assert!((c.penetration - 0.5).abs() < 1e-5);
}

#[test]
fn aabb_picks_axis_of_min_overlap() {
    // Deep x overlap, shallow y overlap — y should win.
    let a = aabb(0.0, 0.0, 5.0, 1.0);
    let b = aabb(0.0, 1.8, 5.0, 1.0);
    let c = aabb_vs_aabb(&a, &b).unwrap();
    assert!(c.normal.y.abs() > c.normal.x.abs());
}

#[test]
fn circle_separated() {
    assert!(circle_vs_circle(Vec2::ZERO, 1.0, Vec2::new(3.0, 0.0), 1.0).is_none());
}

#[test]
fn circle_overlap_normal_points_away_from_b() {
    let c = circle_vs_circle(Vec2::ZERO, 1.0, Vec2::new(1.5, 0.0), 1.0).unwrap();
    // Expected: contact pushes a (at origin) to the left, away from b.
    assert!(c.normal.x < 0.0);
    assert!((c.penetration - 0.5).abs() < 1e-5);
}

#[test]
fn circle_concentric_is_contact() {
    let c = circle_vs_circle(Vec2::ZERO, 1.0, Vec2::ZERO, 1.0).unwrap();
    assert!((c.penetration - 2.0).abs() < 1e-5);
    assert!((c.normal.length() - 1.0).abs() < 1e-5);
}

#[test]
fn aabb_circle_separated() {
    let a = aabb(0.0, 0.0, 1.0, 1.0);
    assert!(aabb_vs_circle(&a, Vec2::new(5.0, 5.0), 1.0).is_none());
}

#[test]
fn aabb_circle_edge_contact() {
    let a = aabb(0.0, 0.0, 1.0, 1.0);
    // Circle center 1.5 right of origin, radius 1.0 → overlap 0.5 on +x.
    // Normal is a→b, i.e. direction the aabb moves to escape the
    // circle, which here is -x.
    let c = aabb_vs_circle(&a, Vec2::new(1.5, 0.0), 1.0).unwrap();
    assert!((c.penetration - 0.5).abs() < 1e-5);
    assert!(c.normal.x < -0.9);
}

#[test]
fn aabb_circle_corner_contact() {
    let a = aabb(0.0, 0.0, 1.0, 1.0);
    // Circle center just outside the +x/+y corner, radius big enough
    // to reach (1,1).
    let c = aabb_vs_circle(&a, Vec2::new(1.3, 1.3), 0.5).unwrap();
    // Normal points from circle back toward the aabb (−x, −y).
    assert!(c.normal.x < 0.0 && c.normal.y < 0.0);
    assert!(c.penetration > 0.0);
}

#[test]
fn aabb_circle_center_inside_picks_nearest_face() {
    let a = aabb(0.0, 0.0, 5.0, 1.0);
    // Circle center slightly above center, nearest face is top (min-y).
    // Pushing the aabb in +y pops the circle out through that face.
    let c = aabb_vs_circle(&a, Vec2::new(0.0, -0.5), 0.1).unwrap();
    assert!(c.normal.y > 0.0);
}

#[test]
fn sweep_hits_static_aabb_in_path() {
    // Moving box at origin half (0.5,0.5), sweeping right toward a
    // static box at (5,0) half (1,1). Expanded target half = (1.5, 1.5),
    // so sweep's x-enter is at center = 5 - 1.5 = 3.5. Starting from
    // x=0 over delta=6, t = 3.5/6 ≈ 0.5833.
    let hit = sweep_aabb_vs_aabb(
        Vec2::new(0.0, 0.0),
        Vec2::new(6.0, 0.0),
        Vec2::new(0.5, 0.5),
        Vec2::new(5.0, 0.0),
        Vec2::new(1.0, 1.0),
    )
    .unwrap();
    assert!((hit.0 - 0.5833).abs() < 1e-3, "t = {}", hit.0);
    assert_eq!(hit.1, Vec2::new(-1.0, 0.0));
}

#[test]
fn sweep_misses_when_offset_from_target() {
    // Same static target but the sweep is offset in y so it never crosses.
    assert!(sweep_aabb_vs_aabb(
        Vec2::new(0.0, 10.0),
        Vec2::new(6.0, 10.0),
        Vec2::new(0.5, 0.5),
        Vec2::new(5.0, 0.0),
        Vec2::new(1.0, 1.0),
    )
    .is_none());
}

#[test]
fn sweep_already_overlapping_returns_none() {
    // Caller is expected to hand already-penetrating pairs to the
    // iteration resolver; the sweep test only catches tunneling along
    // the integration path.
    assert!(sweep_aabb_vs_aabb(
        Vec2::new(0.0, 0.0),
        Vec2::new(1.0, 0.0),
        Vec2::new(0.5, 0.5),
        Vec2::new(0.0, 0.0),
        Vec2::new(1.0, 1.0),
    )
    .is_none());
}

#[test]
fn sweep_picks_entry_axis_for_normal() {
    // Sweep diagonally into the corner of a target expanded to
    // (3.5..6.5) on both axes. delta = (6, 4) means x-slab enters
    // at t = 3.5/6 ≈ 0.583 and y-slab enters at t = 3.5/4 = 0.875;
    // y is the later entry and therefore the contact axis, so the
    // normal is along -y.
    let hit = sweep_aabb_vs_aabb(
        Vec2::new(0.0, 0.0),
        Vec2::new(6.0, 4.0),
        Vec2::new(0.5, 0.5),
        Vec2::new(5.0, 5.0),
        Vec2::new(1.0, 1.0),
    )
    .unwrap();
    assert_eq!(hit.1, Vec2::new(0.0, -1.0));
}

#[test]
fn aabb_overlap_and_union() {
    let a = aabb(0.0, 0.0, 1.0, 1.0);
    let b = aabb(1.5, 0.0, 1.0, 1.0);
    assert!(a.overlaps(&b));
    let u = a.union(&b);
    assert!((u.min().x - (-1.0)).abs() < 1e-5);
    assert!((u.max().x - 2.5).abs() < 1e-5);
}
