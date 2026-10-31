use super::*;
use crate::Pcg32;
use crate::physics::Shape;
use crate::physics::collision::FACE_ALL;

/// The cell every test uses: 16 px square about the origin.
const HALF: Vec2 = Vec2::splat(8.0);

/// The four corners of a box of `half` about `center`, in order.
fn corners(center: Vec2, half: Vec2) -> [Vec2; 4] {
    [
        center + Vec2::new(-half.x, -half.y),
        center + Vec2::new(half.x, -half.y),
        center + Vec2::new(half.x, half.y),
        center + Vec2::new(-half.x, half.y),
    ]
}

/// The distance from `point` to the segment `p`–`q`.
fn segment_distance(point: Vec2, p: Vec2, q: Vec2) -> f32 {
    let t = ((point - p).dot(q - p) / (q - p).length_squared()).clamp(0.0, 1.0);
    point.distance(p + (q - p) * t)
}

/// The polygon's vertices.
fn vertices(poly: &SlopePolygon) -> Vec<Vec2> {
    poly.points[..poly.len].to_vec()
}

/// Each of a closed ring's edges.
fn ring(points: &[Vec2]) -> impl Iterator<Item = (Vec2, Vec2)> + '_ {
    (0..points.len()).map(move |i| (points[i], points[(i + 1) % points.len()]))
}

/// Each edge of a convex ring as its start and its outward unit normal.
fn edges(points: &[Vec2]) -> Vec<(Vec2, Vec2)> {
    let centroid = points.iter().copied().sum::<Vec2>() / points.len() as f32;
    ring(points)
        .map(|(p, q)| {
            let normal = Vec2::new(q.y - p.y, p.x - q.x).normalize();
            let outward = if normal.dot(centroid - p) > 0.0 {
                -normal
            } else {
                normal
            };
            (p, outward)
        })
        .collect()
}

/// The convex hull of `points`, collinear points dropped.
fn hull(mut points: Vec<Vec2>) -> Vec<Vec2> {
    points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    points.dedup();
    let turn = |o: Vec2, a: Vec2, b: Vec2| (a - o).perp_dot(b - o);
    let chain = |points: &mut dyn Iterator<Item = Vec2>| {
        let mut side: Vec<Vec2> = Vec::new();
        for p in points {
            while side.len() >= 2 && turn(side[side.len() - 2], side[side.len() - 1], p) <= 0.0 {
                side.pop();
            }
            side.push(p);
        }
        side.pop();
        side
    };
    let mut out = chain(&mut points.iter().copied());
    out.extend(chain(&mut points.iter().rev().copied()));
    out
}

/// What a shape's centre meets: the polygon itself and the circle's radius,
/// or the polygon's sum with the box and 0.
fn sum(shape: Shape, poly: &SlopePolygon) -> (Vec<Vec2>, f32) {
    match shape {
        Shape::Circle { radius } => (vertices(poly), radius),
        Shape::Aabb { half_extents } => {
            let corners = vertices(poly)
                .iter()
                .flat_map(|&p| corners(p, half_extents))
                .collect();
            (hull(corners), 0.0)
        }
    }
}

/// Whether `point` is strictly inside the convex ring.
fn inside(point: Vec2, points: &[Vec2]) -> bool {
    edges(points)
        .iter()
        .all(|&(p, normal)| (point - p).dot(normal) < 0.0)
}

/// How deep a box overlaps the polygon by SAT on x, y and the slope's
/// normal: the least overlap, 0 or less when apart.
fn box_depth(center: Vec2, half: Vec2, poly: &SlopePolygon) -> f32 {
    [Vec2::X, Vec2::Y, poly.normal]
        .into_iter()
        .map(|axis| {
            let reach = half.x * axis.x.abs() + half.y * axis.y.abs();
            let along: Vec<f32> = vertices(poly).iter().map(|p| p.dot(axis)).collect();
            let low = along.iter().copied().fold(f32::INFINITY, f32::min);
            let high = along.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            (center.dot(axis) + reach).min(high) - (center.dot(axis) - reach).max(low)
        })
        .fold(f32::INFINITY, f32::min)
}

/// The distance from a circle's centre to the polygon's boundary, negative
/// inside.
fn center_distance(center: Vec2, poly: &SlopePolygon) -> f32 {
    let points = vertices(poly);
    let distance = ring(&points)
        .map(|(p, q)| segment_distance(center, p, q))
        .fold(f32::INFINITY, f32::min);
    if inside(center, &points) {
        -distance
    } else {
        distance
    }
}

/// Whether a shape overlaps the polygon, exactly.
fn overlaps(shape: Shape, center: Vec2, poly: &SlopePolygon) -> bool {
    match shape {
        Shape::Circle { radius } => center_distance(center, poly) < radius,
        Shape::Aabb { half_extents } => box_depth(center, half_extents, poly) > 0.0,
    }
}

/// A separated shape's exact distance to the polygon: a circle's from its
/// centre, a box's the least vertex-to-edge distance of either against the
/// other.
fn exact_gap(shape: Shape, center: Vec2, poly: &SlopePolygon) -> f32 {
    match shape {
        Shape::Circle { radius } => center_distance(center, poly) - radius,
        Shape::Aabb { half_extents } => {
            let (shape, points) = (corners(center, half_extents), vertices(poly));
            let one = shape
                .iter()
                .flat_map(|&v| ring(&points).map(move |(p, q)| segment_distance(v, p, q)));
            let two = points
                .iter()
                .flat_map(|&v| ring(&shape).map(move |(p, q)| segment_distance(v, p, q)));
            one.chain(two).fold(f32::INFINITY, f32::min)
        }
    }
}

/// Whether `normal` is an axis side's outward normal that `mask` masks.
fn masked_normal(normal: Vec2, mask: u8) -> bool {
    let face = if normal.abs_diff_eq(Vec2::X, 1e-6) {
        FACE_RIGHT
    } else if normal.abs_diff_eq(Vec2::NEG_X, 1e-6) {
        FACE_LEFT
    } else if normal.abs_diff_eq(Vec2::Y, 1e-6) {
        FACE_BOTTOM
    } else if normal.abs_diff_eq(Vec2::NEG_Y, 1e-6) {
        FACE_TOP
    } else {
        return false;
    };
    mask & face == 0
}

/// Where a separated shape's centre lies against what it meets.
#[derive(Debug, Clone, Copy)]
enum Region {
    /// An exposed edge's inside or a vertex between two exposed edges: the
    /// exact gap, along this normal.
    Exact(Vec2),
    /// A vertex beside one masked edge: the gap along the exposed one's
    /// normal, from its line.
    Clamped(Vec2, f32),
    /// A masked edge's inside or a vertex between two masked edges.
    Masked,
}

/// The region under `mask` of `center`, outside the convex `points`, the
/// shape `offset` past its centre, and how far it lies from the region's
/// bounds in px.
fn region(center: Vec2, points: &[Vec2], offset: f32, mask: u8) -> (Region, f32) {
    let edges = edges(points);
    let n = points.len();
    let (mut best, mut at, mut along) = (f32::INFINITY, 0, 0.0);
    for (i, (p, q)) in ring(points).enumerate() {
        let s = (center - p).dot(q - p) / (q - p).length_squared();
        let distance = center.distance(p + (q - p) * s.clamp(0.0, 1.0));
        if distance < best {
            (best, at, along) = (distance, i, s);
        }
    }
    let exposed = |k: usize| !masked_normal(edges[k].1, mask);
    if along > 0.0 && along < 1.0 {
        let bound = along.min(1.0 - along) * points[at].distance(points[(at + 1) % n]);
        let region = if exposed(at) {
            Region::Exact(edges[at].1)
        } else {
            Region::Masked
        };
        return (region, bound);
    }
    // A vertex, between edge `j` and edge `l` after it.
    let (j, l) = if along <= 0.0 {
        ((at + n - 1) % n, at)
    } else {
        (at, (at + 1) % n)
    };
    let vertex = points[l];
    let direction = |k: usize| (points[(k + 1) % n] - points[k]).normalize();
    let bound = (center - vertex)
        .dot(direction(j))
        .min(-(center - vertex).dot(direction(l)));
    let clamped =
        |k: usize| Region::Clamped(edges[k].1, (center - vertex).dot(edges[k].1) - offset);
    let region = match (exposed(j), exposed(l)) {
        (true, true) => Region::Exact((center - vertex) / best),
        (true, false) => clamped(j),
        (false, true) => clamped(l),
        (false, false) => Region::Masked,
    };
    (region, bound)
}

/// The contact the step computes for a shape against the polygon.
fn contact(
    shape: Shape,
    center: Vec2,
    poly: &SlopePolygon,
    mask: u8,
    margin: f32,
) -> Option<Contact> {
    match shape {
        Shape::Circle { radius } => circle_contact(center, radius, poly, mask, margin),
        Shape::Aabb { half_extents } => box_contact(center, half_extents, poly, mask, margin),
    }
}

/// A seeded box or circle of 1 to 12 px across near the cell.
fn seeded_shape(rng: &mut Pcg32) -> (Shape, Vec2) {
    let shape = if rng.next_u32().is_multiple_of(2) {
        Shape::Aabb {
            half_extents: Vec2::new(rng.next_range(0.5, 6.0), rng.next_range(0.5, 6.0)),
        }
    } else {
        Shape::Circle {
            radius: rng.next_range(0.5, 6.0),
        }
    };
    let center = Vec2::new(rng.next_range(-20.0, 20.0), rng.next_range(-20.0, 20.0));
    (shape, center)
}

#[test]
fn slope_contacts_separate_and_measure_exactly() {
    // Each kind's 2,000 shapes under every mask: the cell alone at
    // `FACE_ALL`, beside masked legs and sides at the others.
    const MARGIN: f32 = 4.0;
    let (mut overlapping, mut near, mut far, mut bounds) = (0, 0, 0, 0);
    for code in 1..=12u8 {
        let poly = polygon(code, Vec2::ZERO, HALF);
        let mut rng = Pcg32::seeded(0x5107_E000 + u64::from(code));
        for probe in 0..2_000 {
            let (shape, center) = seeded_shape(&mut rng);
            let (sum, offset) = sum(shape, &poly);
            let overlap = overlaps(shape, center, &poly);
            let exact = exact_gap(shape, center, &poly);
            for mask in 0..=FACE_ALL {
                let alone = mask == FACE_ALL;
                let at =
                    format!("kind {code}, mask {mask:#06b}, probe {probe}: {shape:?} at {center}");
                let found = contact(shape, center, &poly, mask, MARGIN);
                if let Some(contact) = found {
                    assert!(
                        !masked_normal(contact.normal, mask),
                        "{at}: {contact:?} on a masked side"
                    );
                }
                if overlap {
                    let Some(contact) = found else {
                        assert!(!alone, "{at}: an overlap without a contact");
                        continue;
                    };
                    let out = center + contact.normal * (contact.penetration + 1e-3);
                    assert!(
                        !overlaps(shape, out, &poly),
                        "{at}: {contact:?} still overlaps"
                    );
                    if alone {
                        let short = center + contact.normal * (contact.penetration - 1e-3);
                        assert!(
                            contact.penetration < 1e-3 || overlaps(shape, short, &poly),
                            "{at}: {contact:?} separates short of its penetration"
                        );
                    }
                    overlapping += 1;
                    continue;
                }
                let (region, bound) = region(center, &sum, offset, mask);
                if bound < 1e-3 {
                    // Within rounding of a feature's bounds, where only the
                    // cell alone's gap is continuous.
                    if let (Some(contact), true) = (found, alone) {
                        assert!(
                            (-contact.penetration - exact).abs() < 1e-4,
                            "{at}: {contact:?}, exact {exact}"
                        );
                    }
                    bounds += 1;
                    continue;
                }
                let want = match region {
                    Region::Exact(normal) => Some((normal, exact)),
                    Region::Clamped(normal, gap) => Some((normal, gap)),
                    Region::Masked => None,
                }
                .filter(|&(_, gap)| gap < MARGIN);
                match (found, want) {
                    (Some(contact), Some((normal, gap))) => {
                        let measured = -contact.penetration;
                        assert!(
                            (measured - gap).abs() < 1e-4,
                            "{at}: gap {measured}, want {gap} ({region:?}, exact {exact})"
                        );
                        assert!(
                            contact.normal.abs_diff_eq(normal, 1e-4),
                            "{at}: {contact:?}, want {normal} ({region:?})"
                        );
                        near += 1;
                    }
                    (None, None) => far += 1,
                    (found, want) => {
                        // Either answer within rounding of the margin.
                        let gap = found.map(|c| -c.penetration).or(want.map(|w| w.1));
                        assert!(
                            gap.is_some_and(|gap| (gap - MARGIN).abs() < 1e-4),
                            "{at}: {found:?}, want {want:?} ({region:?}, exact {exact})"
                        );
                    }
                }
            }
        }
    }
    let counts = (overlapping, near, far, bounds);
    assert!(
        overlapping > 20_000 && near > 20_000 && far > 20_000 && bounds < 1_000,
        "{counts:?}"
    );
}

#[test]
fn slope_contacts_clamp_at_a_masked_endpoint() {
    // A box and a circle 1 px out from each vertex where a masked side meets
    // an exposed one, of the polygon or of its sum with the box, along the
    // bisector of their normals.
    let mut met = 0;
    for code in 1..=12u8 {
        let poly = polygon(code, Vec2::ZERO, HALF);
        for shape in [
            Shape::Circle { radius: 2.0 },
            Shape::Aabb {
                half_extents: Vec2::new(2.0, 3.0),
            },
        ] {
            let (sum, offset) = sum(shape, &poly);
            let edges = edges(&sum);
            for mask in 0..FACE_ALL {
                for (l, &vertex) in sum.iter().enumerate() {
                    let j = (l + sum.len() - 1) % sum.len();
                    let (before, after) = (edges[j].1, edges[l].1);
                    let open = match (masked_normal(before, mask), masked_normal(after, mask)) {
                        (false, true) => before,
                        (true, false) => after,
                        _ => continue,
                    };
                    let center = vertex + (before + after).normalize() * (offset + 1.0);
                    let at = format!("kind {code}, mask {mask:#06b}, {shape:?} at {center}");
                    let contact = contact(shape, center, &poly, mask, 8.0)
                        .unwrap_or_else(|| panic!("{at}: no contact"));
                    let gap = (center - vertex).dot(open) - offset;
                    assert!(contact.normal.abs_diff_eq(open, 1e-5), "{at}: {contact:?}");
                    assert!(
                        (-contact.penetration - gap).abs() < 1e-4,
                        "{at}: {contact:?}, {gap} along {open}"
                    );
                    met += 1;
                }
            }
        }
    }
    assert!(met > 200, "{met} masked endpoints met");
}

#[test]
fn a_box_sweep_meets_a_slope_as_its_ray_on_the_sum_does() {
    // The sum built as the hull of every vertex plus every box corner, and a
    // ray of the box's centre clipped by each edge's half plane. Rounding is
    // allowed as distance along an edge's normal: the hit within 1e-3 px of
    // its edge's line and of the sum's first entry, so either edge where two
    // enter together, and either answer where the ray grazes the sum or
    // starts or ends on it.
    const TOL: f32 = 1e-3;
    let mut met = 0;
    for code in 1..=12u8 {
        let poly = polygon(code, Vec2::ZERO, HALF);
        let mut rng = Pcg32::seeded(0x5EE9_0000 + u64::from(code));
        for probe in 0..2_000 {
            let half = Vec2::new(rng.next_range(0.5, 6.0), rng.next_range(0.5, 6.0));
            let from = Vec2::new(rng.next_range(-30.0, 30.0), rng.next_range(-30.0, 30.0));
            let to = Vec2::new(rng.next_range(-30.0, 30.0), rng.next_range(-30.0, 30.0));
            let delta = to - from;
            let sides = edges(&sum(Shape::Aabb { half_extents: half }, &poly).0);
            let (mut enter, mut leave, mut rate) = (f32::NEG_INFINITY, f32::INFINITY, 0.0);
            let mut apart = false;
            for &(p, normal) in &sides {
                let (start, speed) = ((from - p).dot(normal), delta.dot(normal));
                if speed == 0.0 {
                    apart |= start > 0.0;
                } else if speed < 0.0 {
                    if -start / speed > enter {
                        (enter, rate) = (-start / speed, -speed);
                    }
                } else {
                    leave = leave.min(-start / speed);
                }
            }
            let hit = !apart && enter < leave && (0.0..=1.0).contains(&enter);
            let length = delta.length();
            let edge = [leave - enter, enter, 1.0 - enter]
                .iter()
                .any(|t| t.abs() * length < TOL);
            let at = format!("kind {code}, probe {probe}: half {half} from {from} to {to}");
            match box_sweep(from, to, half, &poly) {
                Some((t, normal)) if hit => {
                    let &(p, _) = sides
                        .iter()
                        .find(|(_, side)| side.abs_diff_eq(normal, 1e-4))
                        .unwrap_or_else(|| panic!("{at}: {normal} is no side of the sum"));
                    let point = from + delta * t;
                    assert!(delta.dot(normal) < 0.0, "{at}: left through {normal}");
                    assert!(
                        (point - p).dot(normal).abs() < TOL,
                        "{at}: off its side at {t}"
                    );
                    assert!(
                        (t - enter).abs() * rate < TOL,
                        "{at}: t {t}, the sum's {enter}"
                    );
                    met += 1;
                }
                Some(found) => assert!(edge, "{at}: {found:?}, the sum missed"),
                None => assert!(!hit || edge, "{at}: missed the sum's hit at {enter}"),
            }
        }
    }
    assert!(met > 2_000, "{met} sweeps met their slope");
}
