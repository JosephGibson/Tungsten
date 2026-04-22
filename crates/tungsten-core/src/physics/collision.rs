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

/// Bitmask of exposed faces on an AABB, used for internal-edge filtering
/// when the AABB is a tile in a tilemap and its face is shared with an
/// adjacent solid tile. A clear bit means "this face is internal — any
/// contact that lands on it is spurious (the neighbor tile will generate
/// the correct face contact through its exposed side)". Non-tile colliders
/// pass `FACE_ALL`.
pub const FACE_TOP: u8 = 1 << 0;
pub const FACE_BOTTOM: u8 = 1 << 1;
pub const FACE_LEFT: u8 = 1 << 2;
pub const FACE_RIGHT: u8 = 1 << 3;
pub const FACE_ALL: u8 = FACE_TOP | FACE_BOTTOM | FACE_LEFT | FACE_RIGHT;

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
    aabb_vs_aabb_masked(a, b, FACE_ALL)
}

/// AABB vs AABB with `b`'s face mask respected. When `b` is a tile in a
/// tilemap, MTV axes that point toward an internal (neighbor-shared)
/// face are suppressed so the dynamic body is pushed out through an
/// exposed face instead. If both axes resolve to internal faces, the
/// contact is dropped entirely.
pub fn aabb_vs_aabb_masked(a: &Aabb, b: &Aabb, b_face_mask: u8) -> Option<Contact> {
    let delta = b.center - a.center;
    let overlap_x = (a.half_extents.x + b.half_extents.x) - delta.x.abs();
    let overlap_y = (a.half_extents.y + b.half_extents.y) - delta.y.abs();

    if overlap_x <= 0.0 || overlap_y <= 0.0 {
        return None;
    }

    // Which face of b would each axis's MTV push through? delta = b - a.
    // delta.x < 0 → a is right of b → push along +x clears a through b's
    // RIGHT face; delta.x > 0 → push along -x clears through b's LEFT face.
    let x_face = if delta.x < 0.0 { FACE_RIGHT } else { FACE_LEFT };
    let y_face = if delta.y < 0.0 { FACE_BOTTOM } else { FACE_TOP };
    let x_ok = b_face_mask & x_face != 0;
    let y_ok = b_face_mask & y_face != 0;

    // Pick the smaller overlap unless its face is internal; fall back to
    // the other axis when possible, and skip when neither is exposed.
    let use_x = match (x_ok, y_ok) {
        (false, false) => return None,
        (true, false) => true,
        (false, true) => false,
        (true, true) => overlap_x < overlap_y,
    };

    if use_x {
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
    aabb_vs_circle_masked(aabb, circle_center, radius, FACE_ALL)
}

/// AABB vs circle with face-mask filtering. When the AABB is a tile,
/// contacts whose closest point sits on an internal (neighbor-shared)
/// face are dropped — the adjacent tile generates the correct face
/// contact from its exposed side. Vertex contacts at tile corners
/// require both incident faces to be exposed; otherwise they resolve to
/// diagonal impulses that squeeze bodies through the seam between two
/// tiles. When the circle center is inside the box, exits pick the
/// nearest *exposed* face so an interior path can't be chosen.
pub fn aabb_vs_circle_masked(
    aabb: &Aabb,
    circle_center: Vec2,
    radius: f32,
    face_mask: u8,
) -> Option<Contact> {
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
        //
        // Determine which face(s) the closest point touches. For a pure
        // face contact exactly one of the four bits is set; for a vertex
        // contact two bits are set (the two faces meeting at that
        // corner). Any bit set in `involved` must be an exposed face for
        // this contact to be legitimate; otherwise the neighbor tile on
        // the internal side handles it via a proper face contact.
        let mut involved: u8 = 0;
        if circle_center.x < min.x {
            involved |= FACE_LEFT;
        } else if circle_center.x > max.x {
            involved |= FACE_RIGHT;
        }
        if circle_center.y < min.y {
            involved |= FACE_TOP;
        } else if circle_center.y > max.y {
            involved |= FACE_BOTTOM;
        }
        if involved & !face_mask != 0 {
            return None;
        }

        let dist = dist_sq.sqrt();
        Some(Contact {
            normal: -delta / dist,
            penetration: radius - dist,
        })
    } else {
        // Circle center is inside the box. The shortest exit is along
        // whichever face is nearest; face-mask filtering picks the
        // nearest *exposed* face so we don't push the circle through a
        // neighbor-shared seam into another solid tile.
        let dx_left = circle_center.x - min.x;
        let dx_right = max.x - circle_center.x;
        let dy_top = circle_center.y - min.y;
        let dy_bot = max.y - circle_center.y;

        let mut best_dist = f32::INFINITY;
        let mut best_normal = Vec2::ZERO;
        // Order matches the original tie-break: x-axis first, then y.
        // Pushing the AABB in the +axis direction pops the circle out
        // through the -axis face.
        if face_mask & FACE_LEFT != 0 && dx_left < best_dist {
            best_dist = dx_left;
            best_normal = Vec2::new(1.0, 0.0);
        }
        if face_mask & FACE_RIGHT != 0 && dx_right < best_dist {
            best_dist = dx_right;
            best_normal = Vec2::new(-1.0, 0.0);
        }
        if face_mask & FACE_TOP != 0 && dy_top < best_dist {
            best_dist = dy_top;
            best_normal = Vec2::new(0.0, 1.0);
        }
        if face_mask & FACE_BOTTOM != 0 && dy_bot < best_dist {
            best_dist = dy_bot;
            best_normal = Vec2::new(0.0, -1.0);
        }
        if best_normal == Vec2::ZERO {
            // Fully enclosed tile (all faces internal). A body inside
            // one of these is geometrically unreachable under normal
            // integration; skip rather than push it somewhere worse.
            return None;
        }
        Some(Contact {
            normal: best_normal,
            penetration: best_dist + radius,
        })
    }
}

#[cfg(test)]
#[path = "../tests/physics/collision.rs"]
mod tests;
