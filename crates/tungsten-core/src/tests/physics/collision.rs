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
    // Circle right overlap 0.5; AABB escape normal -x.
    let c = aabb_vs_circle(&a, Vec2::new(1.5, 0.0), 1.0).unwrap();
    assert!((c.penetration - 0.5).abs() < 1e-5);
    assert!(c.normal.x < -0.9);
}

#[test]
fn aabb_circle_corner_contact() {
    let a = aabb(0.0, 0.0, 1.0, 1.0);
    let c = aabb_vs_circle(&a, Vec2::new(1.3, 1.3), 0.5).unwrap();
    assert!(c.normal.x < 0.0 && c.normal.y < 0.0);
    assert!(c.penetration > 0.0);
}

#[test]
fn aabb_circle_center_inside_picks_nearest_face() {
    let a = aabb(0.0, 0.0, 5.0, 1.0);
    // Inside circle uses nearest face normal.
    let c = aabb_vs_circle(&a, Vec2::new(0.0, -0.5), 0.1).unwrap();
    assert!(c.normal.y > 0.0);
}

#[test]
fn sweep_hits_static_aabb_in_path() {
    // Expanded target x-enter: (5 - 1.5) / 6 = 0.5833.
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
    // Already-penetrating pairs are handled by iteration resolver.
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
    // Later entry axis wins: x=0.583, y=0.875 -> -y normal.
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
