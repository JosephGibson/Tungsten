//! Slope tiles' geometry: each kind's polygon in its cell, and a box's and a
//! circle's speculative contact with it and a box's sweep against it, which
//! the step's contacts, its arrival pass and its safety net take.
//!
//! A kind is a code from 1 to 12: its rise (`45`, `2:1 low`, `2:1 high`)
//! times four plus its corner (`br`, `bl`, `tr`, `tl`), plus 1; 0 is a full
//! cell. A polygon's sides along the cell's sides are its legs, which the
//! outline masks as it masks a full cell's faces; its sloped face is always
//! exposed. A box meets the polygon through their Minkowski sum, a pentagon
//! with the normals ±x, ±y and the slope's: a contact's normal is one of
//! them, a masked side's never, and a corner gap beside a masked side takes
//! the exposed side's normal and its projected gap, as
//! [`aabb_vs_aabb_speculative`](crate::physics::aabb_vs_aabb_speculative)
//! does. A circle meets the polygon's nearest point, a vertex beside a
//! masked leg taking the exposed edge's normal and its distance along it, as
//! [`aabb_vs_circle_speculative`](crate::physics::aabb_vs_circle_speculative)
//! does.

use crate::physics::collision::{Contact, FACE_BOTTOM, FACE_LEFT, FACE_RIGHT, FACE_TOP};
use glam::Vec2;

/// A slope cell's polygon: its vertices in order around it, the cell's
/// local corners scaled to the cell, at most four.
#[derive(Debug, Clone, Copy)]
pub(in crate::physics) struct SlopePolygon {
    pub(in crate::physics) points: [Vec2; 4],
    pub(in crate::physics) len: usize,
    /// Each edge's outward unit normal: edge `i` runs from `points[i]` to
    /// the next.
    pub(in crate::physics) normals: [Vec2; 4],
    /// The sloped face's outward unit normal.
    pub(in crate::physics) normal: Vec2,
}

/// The vertices of each kind in a unit cell, y down, by code less 1.
const KINDS: [&[(f32, f32)]; 12] = [
    // 45: br, bl, tr, tl.
    &[(0.0, 1.0), (1.0, 1.0), (1.0, 0.0)],
    &[(0.0, 0.0), (0.0, 1.0), (1.0, 1.0)],
    &[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)],
    &[(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)],
    // 2:1 low: half a cell high.
    &[(0.0, 1.0), (1.0, 1.0), (1.0, 0.5)],
    &[(0.0, 0.5), (0.0, 1.0), (1.0, 1.0)],
    &[(0.0, 0.0), (1.0, 0.0), (1.0, 0.5)],
    &[(0.0, 0.0), (1.0, 0.0), (0.0, 0.5)],
    // 2:1 high: from half a cell to a cell.
    &[(0.0, 1.0), (1.0, 1.0), (1.0, 0.0), (0.0, 0.5)],
    &[(0.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.5)],
    &[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 0.5)],
    &[(0.0, 0.0), (1.0, 0.0), (1.0, 0.5), (0.0, 1.0)],
];

/// The polygon of slope kind `code` (1 to 12) in the cell of `half` around
/// `center`.
pub(in crate::physics) fn polygon(code: u8, center: Vec2, half: Vec2) -> SlopePolygon {
    let local = KINDS[usize::from(code - 1)];
    let min = center - half;
    let size = half * 2.0;
    let mut points = [Vec2::ZERO; 4];
    for (point, &(u, v)) in points.iter_mut().zip(local) {
        *point = min + Vec2::new(u, v) * size;
    }
    let len = local.len();
    // Each edge's normal points away from the polygon's centroid; the one
    // edge along no axis is the sloped face.
    let centroid = points[..len].iter().copied().sum::<Vec2>() / len as f32;
    let mut normals = [Vec2::ZERO; 4];
    let mut normal = Vec2::ZERO;
    for i in 0..len {
        let (p, q) = (points[i], points[(i + 1) % len]);
        let edge = q - p;
        let perp = if p.x == q.x {
            Vec2::X
        } else if p.y == q.y {
            Vec2::Y
        } else {
            Vec2::new(edge.y, -edge.x).normalize()
        };
        normals[i] = if perp.dot(centroid - p) > 0.0 {
            -perp
        } else {
            perp
        };
        if p.x != q.x && p.y != q.y {
            normal = normals[i];
        }
    }
    SlopePolygon {
        points,
        len,
        normals,
        normal,
    }
}

/// The face bit of an axis-aligned outward normal; 0 for the slope's, whose
/// components are at most 2 / √5.
fn face_of(normal: Vec2) -> u8 {
    if normal.x > 0.99 {
        FACE_RIGHT
    } else if normal.x < -0.99 {
        FACE_LEFT
    } else if normal.y > 0.99 {
        FACE_BOTTOM
    } else if normal.y < -0.99 {
        FACE_TOP
    } else {
        0
    }
}

/// Whether a side of outward `normal` may carry a contact under `mask`: the
/// sloped face always, an axis side while its bit is exposed.
fn exposed(normal: Vec2, mask: u8) -> bool {
    let face = face_of(normal);
    face == 0 || mask & face != 0
}

impl SlopePolygon {
    /// The polygon's support point along `dir`.
    fn support(&self, dir: Vec2) -> Vec2 {
        let mut best = self.points[0];
        for &point in &self.points[1..self.len] {
            if point.dot(dir) > best.dot(dir) {
                best = point;
            }
        }
        best
    }

    /// Edge `i`: from `points[i]` to the next, with its outward normal.
    fn edge(&self, i: usize) -> (Vec2, Vec2, Vec2) {
        (
            self.points[i],
            self.points[(i + 1) % self.len],
            self.normals[i],
        )
    }

    /// Whether `point` lies strictly inside the polygon.
    pub(in crate::physics) fn contains(&self, point: Vec2) -> bool {
        (0..self.len).all(|i| {
            let (p, _, normal) = self.edge(i);
            (point - p).dot(normal) < 0.0
        })
    }

    /// The Minkowski sum with a box of `half`: five vertices, each edge `k`
    /// running from vertex `k` to the next with outward normal `normals[k]`
    /// in angle order, y down.
    fn minkowski(&self, half: Vec2) -> ([Vec2; 5], [Vec2; 5]) {
        let mut normals = [Vec2::X, Vec2::Y, Vec2::NEG_X, Vec2::NEG_Y, self.normal];
        let angle = |n: Vec2| n.y.atan2(n.x).rem_euclid(std::f32::consts::TAU);
        normals.sort_by(|a, b| angle(*a).total_cmp(&angle(*b)));
        let mut vertices = [Vec2::ZERO; 5];
        for (k, vertex) in vertices.iter_mut().enumerate() {
            // The vertex before edge `k`: between it and the edge before.
            let between = normals[(k + 4) % 5] + normals[k];
            let corner = Vec2::new(half.x.copysign(between.x), half.y.copysign(between.y));
            *vertex = self.support(between) + corner;
        }
        (vertices, normals)
    }
}

/// The signed-distance contact of a box of `half` at `center` against the
/// slope polygon `poly` whose exposed sides are `mask` (`D-064`): an overlap
/// pushes out along the least exposed side of their Minkowski sum; a gap
/// under `margin` is its distance to the nearest feature, negative, a masked
/// side never its normal and a corner beside one clamped to the exposed
/// side's projected gap. `None` past the margin or when every side near is
/// masked.
pub(in crate::physics) fn box_contact(
    center: Vec2,
    half: Vec2,
    poly: &SlopePolygon,
    mask: u8,
    margin: f32,
) -> Option<Contact> {
    let (vertices, normals) = poly.minkowski(half);
    let reach = |k: usize| vertices[k].dot(normals[k]);
    let separation = |k: usize| center.dot(normals[k]) - reach(k);
    if (0..5).all(|k| separation(k) <= 0.0) {
        let k = (0..5)
            .filter(|&k| exposed(normals[k], mask))
            .min_by(|&a, &b| (-separation(a)).total_cmp(&-separation(b)))?;
        return Some(Contact {
            normal: normals[k],
            penetration: -separation(k),
        });
    }
    // The nearest feature of the sum: an edge's inside or a vertex.
    let mut best: Option<(f32, usize, f32)> = None;
    for k in 0..5 {
        let (p, q) = (vertices[k], vertices[(k + 1) % 5]);
        let along = (center - p).dot(q - p) / (q - p).length_squared();
        let t = along.clamp(0.0, 1.0);
        let distance = center.distance(p + (q - p) * t);
        if best.is_none_or(|(kept, ..)| distance < kept) {
            best = Some((distance, k, t));
        }
    }
    let (distance, k, t) = best?;
    let contact = if t > 0.0 && t < 1.0 {
        if !exposed(normals[k], mask) {
            return None;
        }
        Contact {
            normal: normals[k],
            penetration: -separation(k),
        }
    } else {
        // A vertex, between edge `j` and edge `l` after it.
        let (j, l) = if t <= 0.0 {
            ((k + 4) % 5, k)
        } else {
            (k, (k + 1) % 5)
        };
        let vertex = vertices[l];
        match (exposed(normals[j], mask), exposed(normals[l], mask)) {
            (true, true) => Contact {
                normal: (center - vertex) / distance,
                penetration: -distance,
            },
            (true, false) => Contact {
                normal: normals[j],
                penetration: -separation(j),
            },
            (false, true) => Contact {
                normal: normals[l],
                penetration: -separation(l),
            },
            (false, false) => return None,
        }
    };
    (contact.penetration > -margin).then_some(contact)
}

/// The signed-distance contact of a circle of `radius` at `center` against
/// the slope polygon `poly` whose exposed sides are `mask` (`D-064`): from
/// the polygon's nearest point, a centre inside pushing out through the
/// nearest exposed side, a masked leg's inside dropping and a vertex beside
/// one clamped to the exposed edge's normal and its distance along it. A
/// gap under `margin` is negative. `None` past the margin or when every
/// side near is masked.
pub(in crate::physics) fn circle_contact(
    center: Vec2,
    radius: f32,
    poly: &SlopePolygon,
    mask: u8,
    margin: f32,
) -> Option<Contact> {
    let line = |i: usize| {
        let (p, _, normal) = poly.edge(i);
        (normal, (center - p).dot(normal))
    };
    if poly.contains(center) {
        let i = (0..poly.len)
            .filter(|&i| exposed(line(i).0, mask))
            .min_by(|&a, &b| (-line(a).1).total_cmp(&-line(b).1))?;
        let (normal, signed) = line(i);
        return Some(Contact {
            normal,
            penetration: radius - signed,
        });
    }
    let mut best: Option<(f32, usize, f32)> = None;
    for i in 0..poly.len {
        let (p, q, _) = poly.edge(i);
        let t = ((center - p).dot(q - p) / (q - p).length_squared()).clamp(0.0, 1.0);
        let distance = center.distance(p + (q - p) * t);
        if best.is_none_or(|(kept, ..)| distance < kept) {
            best = Some((distance, i, t));
        }
    }
    let (distance, i, t) = best?;
    let contact = if t > 0.0 && t < 1.0 {
        let (normal, signed) = line(i);
        if !exposed(normal, mask) {
            return None;
        }
        Contact {
            normal,
            penetration: radius - signed,
        }
    } else {
        let (j, l) = if t <= 0.0 {
            ((i + poly.len - 1) % poly.len, i)
        } else {
            (i, (i + 1) % poly.len)
        };
        let vertex = poly.points[l];
        match (exposed(line(j).0, mask), exposed(line(l).0, mask)) {
            (true, true) => Contact {
                normal: (center - vertex) / distance,
                penetration: radius - distance,
            },
            (true, false) => Contact {
                normal: line(j).0,
                penetration: radius - line(j).1,
            },
            (false, true) => Contact {
                normal: line(l).0,
                penetration: radius - line(l).1,
            },
            (false, false) => return None,
        }
    };
    (contact.penetration > -margin).then_some(contact)
}

/// A box of `half` moving from `from` to `to` against the slope polygon:
/// the `t` in `[0, 1]` it first touches it and the side's outward normal, as
/// a ray of its centre meets their Minkowski sum; `None` when it misses or
/// starts overlapping.
pub(in crate::physics) fn box_sweep(
    from: Vec2,
    to: Vec2,
    half: Vec2,
    poly: &SlopePolygon,
) -> Option<(f32, Vec2)> {
    let delta = to - from;
    if delta == Vec2::ZERO {
        return None;
    }
    let (vertices, normals) = poly.minkowski(half);
    let (mut enter, mut leave) = (f32::NEG_INFINITY, f32::INFINITY);
    let mut entered = None;
    for k in 0..5 {
        let normal = normals[k];
        let start = (from - vertices[k]).dot(normal);
        let rate = delta.dot(normal);
        if rate.abs() < f32::EPSILON {
            if start > 0.0 {
                return None;
            }
            continue;
        }
        let t = -start / rate;
        if rate < 0.0 {
            if t > enter {
                enter = t;
                entered = Some(normal);
            }
        } else {
            leave = leave.min(t);
        }
    }
    let normal = entered?;
    (enter < leave && (0.0..=1.0).contains(&enter)).then_some((enter, normal))
}

/// Slope kind `code`'s vertex farthest out across its cell side `side`, in
/// the cell's local units, y down: where a box across a side without a leg
/// meets the polygon.
pub(in crate::physics) fn apex(code: u8, side: u8) -> (f32, f32) {
    let out = |&(u, v): &(f32, f32)| match side {
        FACE_TOP => -v,
        FACE_BOTTOM => v,
        FACE_LEFT => -u,
        _ => u,
    };
    let local = KINDS[usize::from(code - 1)];
    let mut best = local[0];
    for point in &local[1..] {
        if out(point) > out(&best) {
            best = *point;
        }
    }
    best
}

/// Whether `(u, v)`, in the cell's local units, lies strictly inside slope
/// kind `code`'s polygon.
pub(in crate::physics) fn holds(code: u8, u: f32, v: f32) -> bool {
    polygon(code, Vec2::splat(0.5), Vec2::splat(0.5)).contains(Vec2::new(u, v))
}

/// The cell side bits slope kind `code` has a leg on, and each leg's span
/// along its side in the cell's local units, from its left or top end.
pub(in crate::physics) fn legs(code: u8) -> [(u8, f32, f32); 3] {
    let local = KINDS[usize::from(code - 1)];
    let mut out = [(0, 0.0, 0.0); 3];
    let mut at = 0;
    for i in 0..local.len() {
        let ((pu, pv), (qu, qv)) = (local[i], local[(i + 1) % local.len()]);
        let leg = if pu == qu && (pu == 0.0 || pu == 1.0) {
            Some((
                if pu == 0.0 { FACE_LEFT } else { FACE_RIGHT },
                pv.min(qv),
                pv.max(qv),
            ))
        } else if pv == qv && (pv == 0.0 || pv == 1.0) {
            Some((
                if pv == 0.0 { FACE_TOP } else { FACE_BOTTOM },
                pu.min(qu),
                pu.max(qu),
            ))
        } else {
            None
        };
        if let Some(leg) = leg
            && at < out.len()
        {
            out[at] = leg;
            at += 1;
        }
    }
    out
}

#[cfg(test)]
#[path = "../../tests/physics/slope.rs"]
mod tests;
