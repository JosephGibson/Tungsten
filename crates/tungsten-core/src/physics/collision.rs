//! Narrow-phase shape-vs-shape collision tests.
//!
//! All three pair types return an `Option<Contact>`: `None` when the
//! shapes are separated, `Some` when they overlap. The contact carries
//! a unit `normal` pointing from `a` into `b`'s free space and the
//! `penetration` depth along that normal — enough to perform a
//! minimum-translation-vector resolution at the call site.
//!
//! Shapes in M11 are all axis-aligned, so full SAT isn't needed: AABB
//! separation reduces to per-axis overlap, circle separation reduces
//! to a distance check, and AABB-vs-circle reduces to a closest-point
//! test. The SAT generalization is the same idea (project onto candidate
//! axes, pick the axis of minimum overlap) — documented here rather than
//! implemented because M11 has no rotating shapes.

use glam::Vec2;

/// A resolved contact between two shapes. `normal` points from `a`
/// toward `b`'s free space (i.e. the direction `a` should move to
/// leave `b`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Contact {
    pub normal: Vec2,
    pub penetration: f32,
}

/// An axis-aligned bounding box in world space, expressed as center
/// and half-extents. This is the canonical broad-phase proxy for every
/// collider type — circles promote to their bounding square.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub center: Vec2,
    pub half_extents: Vec2,
}

impl Aabb {
    pub fn new(center: Vec2, half_extents: Vec2) -> Self {
        Self {
            center,
            half_extents,
        }
    }

    /// Construct from min/max corners.
    pub fn from_min_max(min: Vec2, max: Vec2) -> Self {
        let center = (min + max) * 0.5;
        let half_extents = (max - min) * 0.5;
        Self {
            center,
            half_extents,
        }
    }

    pub fn min(&self) -> Vec2 {
        self.center - self.half_extents
    }

    pub fn max(&self) -> Vec2 {
        self.center + self.half_extents
    }

    /// Expand this AABB to also contain `other`. Used by the broad-phase
    /// when a shape overlaps multiple grid cells.
    pub fn union(&self, other: &Aabb) -> Aabb {
        let min = self.min().min(other.min());
        let max = self.max().max(other.max());
        Aabb::from_min_max(min, max)
    }

    /// Quick overlap test. Used for broad-phase filtering.
    pub fn overlaps(&self, other: &Aabb) -> bool {
        let a_min = self.min();
        let a_max = self.max();
        let b_min = other.min();
        let b_max = other.max();
        a_max.x > b_min.x && a_min.x < b_max.x && a_max.y > b_min.y && a_min.y < b_max.y
    }
}

/// AABB vs AABB. Returns the minimum-translation-vector on the axis of
/// smallest overlap, with the normal pointing from `a` toward `b`'s
/// free space. Touching (`overlap == 0`) is treated as separated so
/// sliding along a wall doesn't generate spurious events.
pub fn aabb_vs_aabb(a: &Aabb, b: &Aabb) -> Option<Contact> {
    let delta = b.center - a.center;
    let overlap_x = (a.half_extents.x + b.half_extents.x) - delta.x.abs();
    let overlap_y = (a.half_extents.y + b.half_extents.y) - delta.y.abs();

    if overlap_x <= 0.0 || overlap_y <= 0.0 {
        return None;
    }

    // Minimum-translation axis — the smaller overlap is the shorter
    // push out of the penetration.
    if overlap_x < overlap_y {
        let sign = if delta.x < 0.0 { 1.0 } else { -1.0 };
        Some(Contact {
            normal: Vec2::new(sign, 0.0),
            penetration: overlap_x,
        })
    } else {
        let sign = if delta.y < 0.0 { 1.0 } else { -1.0 };
        Some(Contact {
            normal: Vec2::new(0.0, sign),
            penetration: overlap_y,
        })
    }
}

/// Circle vs circle. Treats concentric circles as a contact with a
/// position-derived escape normal rather than emitting a zero-normal
/// event, because a zero normal can't be used by the MTV resolver
/// downstream.
pub fn circle_vs_circle(
    center_a: Vec2,
    radius_a: f32,
    center_b: Vec2,
    radius_b: f32,
) -> Option<Contact> {
    let delta = center_b - center_a;
    let dist_sq = delta.length_squared();
    let r_sum = radius_a + radius_b;
    if dist_sq >= r_sum * r_sum {
        return None;
    }
    let dist = dist_sq.sqrt();
    let (normal, penetration) = if dist > f32::EPSILON {
        // Normal points from a toward b, then negated so it pushes
        // a out of b (i.e. from b's interior back to a).
        (-delta / dist, r_sum - dist)
    } else {
        // Degenerate: concentric. A fixed normal (the old `(-1, 0)`)
        // pushes every coincident pair the same way, so a pile of
        // coincident bodies all drift in that direction each substep.
        // Hash the position bits into an angle so different concentric
        // pairs escape along different axes; still deterministic for
        // reproducibility.
        let bits = center_a.x.to_bits() ^ center_a.y.to_bits().rotate_left(13);
        let angle = (bits as f32) * (std::f32::consts::TAU / u32::MAX as f32);
        (Vec2::new(angle.cos(), angle.sin()), r_sum)
    };
    Some(Contact {
        normal,
        penetration,
    })
}

/// Swept AABB vs static AABB. Casts the path of a moving AABB (from
/// `a_prev` to `a_cur`, half-extents `a_half`) against a stationary
/// target AABB (`b_center`, `b_half`). Returns `Some((t, normal))`
/// where `t ∈ [0, 1]` is the fraction of the sweep at first contact
/// and `normal` points from the contact face back toward `a`'s free
/// space (same convention as `aabb_vs_aabb`). Returns `None` if the
/// path doesn't reach the target in `[0, 1]` or if they already
/// overlap at `t = 0` (leave overlapping cases to the regular
/// narrow-phase resolver).
///
/// Used by the speculative-contact path in `physics_step` to catch
/// tunneling when the substep cap binds. Circles against static AABBs
/// reuse this by approximating the circle as a point and expanding
/// the target by the circle's radius; the corner-rounding error is
/// conservative (may trigger slightly early) which is the safe
/// direction for tunneling prevention.
pub fn sweep_aabb_vs_aabb(
    a_prev: Vec2,
    a_cur: Vec2,
    a_half: Vec2,
    b_center: Vec2,
    b_half: Vec2,
) -> Option<(f32, Vec2)> {
    let delta = a_cur - a_prev;
    if delta == Vec2::ZERO {
        return None;
    }
    let expanded_half = a_half + b_half;
    let min = b_center - expanded_half;
    let max = b_center + expanded_half;

    let (tx_near, tx_far) = sweep_slab(a_prev.x, delta.x, min.x, max.x)?;
    let (ty_near, ty_far) = sweep_slab(a_prev.y, delta.y, min.y, max.y)?;

    let t_near = tx_near.max(ty_near);
    let t_far = tx_far.min(ty_far);

    // No overlap on the combined interval, out of reach this sweep,
    // or already overlapping at t=0 (let the iteration resolver handle it).
    if t_near >= t_far || !(0.0..=1.0).contains(&t_near) {
        return None;
    }

    let normal = if tx_near > ty_near {
        Vec2::new(if delta.x > 0.0 { -1.0 } else { 1.0 }, 0.0)
    } else {
        Vec2::new(0.0, if delta.y > 0.0 { -1.0 } else { 1.0 })
    };
    Some((t_near, normal))
}

fn sweep_slab(prev: f32, delta: f32, min: f32, max: f32) -> Option<(f32, f32)> {
    if delta.abs() < f32::EPSILON {
        if prev < min || prev > max {
            return None;
        }
        return Some((f32::NEG_INFINITY, f32::INFINITY));
    }
    let inv = 1.0 / delta;
    let t1 = (min - prev) * inv;
    let t2 = (max - prev) * inv;
    Some((t1.min(t2), t1.max(t2)))
}

/// AABB vs circle. Finds the closest point on the box to the circle
/// center, then treats that point as the contact location. Handles
/// both edge, corner, and circle-inside-box cases.
pub fn aabb_vs_circle(aabb: &Aabb, circle_center: Vec2, radius: f32) -> Option<Contact> {
    let min = aabb.min();
    let max = aabb.max();
    let closest = Vec2::new(
        circle_center.x.clamp(min.x, max.x),
        circle_center.y.clamp(min.y, max.y),
    );
    let delta = circle_center - closest;
    let dist_sq = delta.length_squared();

    if dist_sq >= radius * radius && dist_sq > 0.0 {
        return None;
    }

    if dist_sq > f32::EPSILON {
        // Circle outside the box (or touching a face/corner): contact
        // normal is a→b, i.e. the direction the AABB should move to
        // escape the circle, which points from the circle back toward
        // the box's closest point.
        let dist = dist_sq.sqrt();
        Some(Contact {
            normal: -delta / dist,
            penetration: radius - dist,
        })
    } else {
        // Circle center is inside the box. The shortest exit is along
        // whichever face is nearest. Compute penetration on each axis
        // and take the smaller one. Pushing the AABB in the +axis direction
        // pops the circle out through the -axis face, so when the circle
        // is closer to the low-side face we push the box toward +axis.
        let dx_left = circle_center.x - min.x;
        let dx_right = max.x - circle_center.x;
        let dy_top = circle_center.y - min.y;
        let dy_bot = max.y - circle_center.y;
        let min_x = dx_left.min(dx_right);
        let min_y = dy_top.min(dy_bot);
        if min_x < min_y {
            let sign = if dx_left < dx_right { 1.0 } else { -1.0 };
            Some(Contact {
                normal: Vec2::new(sign, 0.0),
                penetration: min_x + radius,
            })
        } else {
            let sign = if dy_top < dy_bot { 1.0 } else { -1.0 };
            Some(Contact {
                normal: Vec2::new(0.0, sign),
                penetration: min_y + radius,
            })
        }
    }
}

#[cfg(test)]
mod tests {
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
}
