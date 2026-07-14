//! Axis-aligned narrow phase; contact normals move `a` out of `b`.

use glam::Vec2;

/// AABB exposed-face mask for tile internal-edge filtering.
pub const FACE_TOP: u8 = 1 << 0;
pub const FACE_BOTTOM: u8 = 1 << 1;
pub const FACE_LEFT: u8 = 1 << 2;
pub const FACE_RIGHT: u8 = 1 << 3;
pub const FACE_ALL: u8 = FACE_TOP | FACE_BOTTOM | FACE_LEFT | FACE_RIGHT;

/// Contact normal moves `a` out of `b`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Contact {
    pub normal: Vec2,
    pub penetration: f32,
}

/// World-space AABB; circles promote to bounding square for broadphase.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub center: Vec2,
    pub half_extents: Vec2,
}

impl Aabb {
    #[must_use]
    pub fn new(center: Vec2, half_extents: Vec2) -> Self {
        Self {
            center,
            half_extents,
        }
    }

    /// Construct from min/max corners.
    #[must_use]
    pub fn from_min_max(min: Vec2, max: Vec2) -> Self {
        let center = (min + max) * 0.5;
        let half_extents = (max - min) * 0.5;
        Self {
            center,
            half_extents,
        }
    }

    #[must_use]
    pub fn min(&self) -> Vec2 {
        self.center - self.half_extents
    }

    #[must_use]
    pub fn max(&self) -> Vec2 {
        self.center + self.half_extents
    }

    /// Bounding union.
    #[must_use]
    pub fn union(&self, other: &Aabb) -> Aabb {
        let min = self.min().min(other.min());
        let max = self.max().max(other.max());
        Aabb::from_min_max(min, max)
    }

    /// Strict overlap test.
    #[must_use]
    pub fn overlaps(&self, other: &Aabb) -> bool {
        let a_min = self.min();
        let a_max = self.max();
        let b_min = other.min();
        let b_max = other.max();
        a_max.x > b_min.x && a_min.x < b_max.x && a_max.y > b_min.y && a_min.y < b_max.y
    }
}

/// AABB vs AABB; touching is separated.
#[must_use]
pub fn aabb_vs_aabb(a: &Aabb, b: &Aabb) -> Option<Contact> {
    aabb_vs_aabb_masked(a, b, FACE_ALL)
}

/// AABB vs AABB with `b` internal faces suppressed.
#[must_use]
pub fn aabb_vs_aabb_masked(a: &Aabb, b: &Aabb, b_face_mask: u8) -> Option<Contact> {
    aabb_vs_aabb_speculative(a, b, b_face_mask, 0.0)
}

/// Signed-distance AABB vs AABB (D-064): admits separated pairs with a gap
/// under `margin` as contacts with **negative** `penetration` (= -gap).
/// Corner-region gaps produce a diagonal normal; when one axis of the corner
/// points at an internal `b` face, the normal clamps to the exposed axis
/// (tile-seam ghost-collision mitigation). `margin = 0` reproduces
/// [`aabb_vs_aabb_masked`] exactly.
#[must_use]
pub fn aabb_vs_aabb_speculative(
    a: &Aabb,
    b: &Aabb,
    b_face_mask: u8,
    margin: f32,
) -> Option<Contact> {
    let delta = b.center - a.center;
    // Signed per-axis separation; positive = gap on that axis.
    let sep_x = delta.x.abs() - (a.half_extents.x + b.half_extents.x);
    let sep_y = delta.y.abs() - (a.half_extents.y + b.half_extents.y);

    // Map each axis to the `b` face bit it would contact.
    let x_face = if delta.x < 0.0 { FACE_RIGHT } else { FACE_LEFT };
    let y_face = if delta.y < 0.0 { FACE_BOTTOM } else { FACE_TOP };
    let x_ok = b_face_mask & x_face != 0;
    let y_ok = b_face_mask & y_face != 0;
    let x_normal = Vec2::new(if delta.x < 0.0 { 1.0 } else { -1.0 }, 0.0);
    let y_normal = Vec2::new(0.0, if delta.y < 0.0 { 1.0 } else { -1.0 });

    let contact = if sep_x <= 0.0 && sep_y <= 0.0 {
        // Overlapping: smallest exposed overlap wins; fully internal drops.
        let use_x = match (x_ok, y_ok) {
            (false, false) => return None,
            (true, false) => true,
            (false, true) => false,
            (true, true) => sep_x > sep_y,
        };
        if use_x {
            Contact {
                normal: x_normal,
                penetration: -sep_x,
            }
        } else {
            Contact {
                normal: y_normal,
                penetration: -sep_y,
            }
        }
    } else if sep_x > 0.0 && sep_y > 0.0 {
        // Corner region: diagonal normal, clamped against internal faces.
        match (x_ok, y_ok) {
            (false, false) => return None,
            (true, false) => Contact {
                normal: x_normal,
                penetration: -sep_x,
            },
            (false, true) => Contact {
                normal: y_normal,
                penetration: -sep_y,
            },
            (true, true) => {
                let gap = Vec2::new(sep_x, sep_y).length();
                Contact {
                    normal: (x_normal * sep_x + y_normal * sep_y) / gap,
                    penetration: -gap,
                }
            }
        }
    } else if sep_x > 0.0 {
        // Face gap along x; internal face never generates a gap contact.
        if !x_ok {
            return None;
        }
        Contact {
            normal: x_normal,
            penetration: -sep_x,
        }
    } else {
        if !y_ok {
            return None;
        }
        Contact {
            normal: y_normal,
            penetration: -sep_y,
        }
    };

    (contact.penetration > -margin).then_some(contact)
}

/// Circle vs circle; concentric pairs get deterministic nonzero normal.
#[must_use]
pub fn circle_vs_circle(
    center_a: Vec2,
    radius_a: f32,
    center_b: Vec2,
    radius_b: f32,
) -> Option<Contact> {
    circle_vs_circle_speculative(center_a, radius_a, center_b, radius_b, 0.0)
}

/// Signed-distance circle vs circle (D-064): admits separated pairs with a
/// gap under `margin` as contacts with negative `penetration` (= -gap).
/// `margin = 0` reproduces [`circle_vs_circle`] exactly.
#[must_use]
pub fn circle_vs_circle_speculative(
    center_a: Vec2,
    radius_a: f32,
    center_b: Vec2,
    radius_b: f32,
    margin: f32,
) -> Option<Contact> {
    let delta = center_b - center_a;
    let dist_sq = delta.length_squared();
    let r_sum = radius_a + radius_b;
    let reach = r_sum + margin;
    if dist_sq >= reach * reach {
        return None;
    }
    let dist = dist_sq.sqrt();
    let (normal, penetration) = if dist > f32::EPSILON {
        (-delta / dist, r_sum - dist)
    } else {
        // Hash position bits to avoid same-axis drift for coincident piles.
        let bits = center_a.x.to_bits() ^ center_a.y.to_bits().rotate_left(13);
        let angle = (bits as f32) * (std::f32::consts::TAU / u32::MAX as f32);
        (Vec2::new(angle.cos(), angle.sin()), r_sum)
    };
    Some(Contact {
        normal,
        penetration,
    })
}

/// Swept moving AABB vs static AABB; overlap at `t = 0` is ignored.
#[must_use]
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

    // Exclude misses and already-overlapping starts.
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

/// AABB vs circle using closest point.
#[must_use]
pub fn aabb_vs_circle(aabb: &Aabb, circle_center: Vec2, radius: f32) -> Option<Contact> {
    aabb_vs_circle_masked(aabb, circle_center, radius, FACE_ALL)
}

/// AABB vs circle with tile internal-face filtering.
#[must_use]
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
        // Closest-point face bits must all be exposed; vertex needs both faces.
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
        aabb_circle_deep_center(aabb, circle_center, radius, face_mask)
    }
}

/// Signed-distance AABB vs circle (D-064): admits a separated circle with a
/// gap under `margin` as a contact with negative `penetration` (= -gap).
/// Vertex-region contacts whose corner touches an internal face clamp the
/// normal to the exposed axis (tile-seam ghost-collision mitigation) instead
/// of dropping, so speculative CCD never loses a seam contact; a pure face
/// contact on an internal face still drops.
#[must_use]
pub fn aabb_vs_circle_speculative(
    aabb: &Aabb,
    circle_center: Vec2,
    radius: f32,
    face_mask: u8,
    margin: f32,
) -> Option<Contact> {
    let min = aabb.min();
    let max = aabb.max();
    let closest = Vec2::new(
        circle_center.x.clamp(min.x, max.x),
        circle_center.y.clamp(min.y, max.y),
    );
    let delta = circle_center - closest;
    let dist_sq = delta.length_squared();

    if dist_sq <= f32::EPSILON {
        // Circle center inside the box: always deeply penetrating.
        return aabb_circle_deep_center(aabb, circle_center, radius, face_mask);
    }

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

    let contact = if involved & !face_mask == 0 {
        let dist = dist_sq.sqrt();
        Contact {
            normal: -delta / dist,
            penetration: radius - dist,
        }
    } else {
        // Clamp to the single exposed axis of a vertex contact; per-axis
        // distance from the circle center to that face plane is the gap base.
        let (normal, axial) = match involved & face_mask {
            FACE_LEFT => (Vec2::new(1.0, 0.0), min.x - circle_center.x),
            FACE_RIGHT => (Vec2::new(-1.0, 0.0), circle_center.x - max.x),
            FACE_TOP => (Vec2::new(0.0, 1.0), min.y - circle_center.y),
            FACE_BOTTOM => (Vec2::new(0.0, -1.0), circle_center.y - max.y),
            _ => return None,
        };
        Contact {
            normal,
            penetration: radius - axial,
        }
    };

    (contact.penetration > -margin).then_some(contact)
}

/// Nearest exposed face for a circle whose center lies inside the AABB.
fn aabb_circle_deep_center(
    aabb: &Aabb,
    circle_center: Vec2,
    radius: f32,
    face_mask: u8,
) -> Option<Contact> {
    let min = aabb.min();
    let max = aabb.max();
    let dx_left = circle_center.x - min.x;
    let dx_right = max.x - circle_center.x;
    let dy_top = circle_center.y - min.y;
    let dy_bot = max.y - circle_center.y;

    let mut best_dist = f32::INFINITY;
    let mut best_normal = Vec2::ZERO;
    // Tie-break: x-axis before y-axis.
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
        return None;
    }
    Some(Contact {
        normal: best_normal,
        penetration: best_dist + radius,
    })
}

#[cfg(test)]
#[path = "../tests/physics/collision.rs"]
mod tests;
