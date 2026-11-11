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

/// Whether the vertex between the exposed side of outward normal `open`
/// and the masked one of `leg` is an open corner: an axis side strictly
/// between their normals that `mask` exposes, as the outline exposes a side
/// without a leg while no neighbour stands past the vertex.
fn open_corner(open: Vec2, leg: Vec2, mask: u8) -> bool {
    let turn = open.perp_dot(leg);
    [Vec2::X, Vec2::Y, Vec2::NEG_X, Vec2::NEG_Y]
        .into_iter()
        .any(|side| {
            open.perp_dot(side) * turn > 0.0
                && side.perp_dot(leg) * turn > 0.0
                && !masked_normal(side, mask)
        })
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
    /// A masked edge's inside, a vertex between two masked edges, or an
    /// open corner past its masked edge's line.
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
    let mut bound = (center - vertex)
        .dot(direction(j))
        .min(-(center - vertex).dot(direction(l)));
    let mut clamped = |open: usize, leg: usize| {
        let (normal, leg) = (edges[open].1, edges[leg].1);
        if !open_corner(normal, leg, mask) {
            return Region::Clamped(normal, (center - vertex).dot(normal) - offset);
        }
        // An open corner: the vertex on the exposed side's side of the
        // masked edge's line, nothing past it.
        let past = (center - vertex).dot(leg);
        bound = bound.min(past.abs());
        if past > 0.0 {
            Region::Masked
        } else {
            Region::Exact((center - vertex) / best)
        }
    };
    let region = match (exposed(j), exposed(l)) {
        (true, true) => Region::Exact((center - vertex) / best),
        (true, false) => clamped(j, l),
        (false, true) => clamped(l, j),
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
    // bisector of their normals; at an open corner, which only a circle's
    // polygon has, that lies past the masked side's line and meets nothing.
    let (mut met, mut open_met) = (0, 0);
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
                    let (open, leg) =
                        match (masked_normal(before, mask), masked_normal(after, mask)) {
                            (false, true) => (before, after),
                            (true, false) => (after, before),
                            _ => continue,
                        };
                    let center = vertex + (before + after).normalize() * (offset + 1.0);
                    let at = format!("kind {code}, mask {mask:#06b}, {shape:?} at {center}");
                    let contact = contact(shape, center, &poly, mask, 8.0);
                    if open_corner(open, leg, mask) {
                        assert!(contact.is_none(), "{at}: {contact:?} at an open corner");
                        open_met += 1;
                        continue;
                    }
                    let contact = contact.unwrap_or_else(|| panic!("{at}: no contact"));
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
    assert!(
        met > 200 && open_met > 20,
        "{met} masked endpoints met, {open_met} open corners"
    );
}

#[test]
fn a_zero_height_box_meets_a_slope_face() {
    // A box of half (4, 0) 1.414 px over a 45 bl cell's face: at a zero
    // extent the sum's first edge has zero length, and its 0 / 0 once
    // seeded the nearest-feature search, losing the contact or reading a
    // false deep one under a masked side.
    let poly = polygon(2, Vec2::splat(8.0), HALF);
    for mask in [FACE_ALL, FACE_ALL & !FACE_BOTTOM, FACE_ALL & !FACE_RIGHT] {
        let contact = box_contact(Vec2::new(10.0, 4.0), Vec2::new(4.0, 0.0), &poly, mask, 4.0);
        assert!(
            contact.is_some_and(|c| c.normal.abs_diff_eq(poly.normal, 1e-4)
                && (c.penetration + std::f32::consts::SQRT_2).abs() < 1e-3),
            "mask {mask:#06b}: {contact:?}"
        );
    }
}

#[test]
fn a_zero_height_box_by_a_ramps_foot_meets_the_face() {
    // By a ramp's foot a box of zero height has the sum's foot vertex
    // twice, its zero-length side between them; skipping that side left the
    // vertex to a tie that named the wrong sides, so no contact or a false
    // one came back. 0.5 px over the floor and 2 px from the foot, the
    // centre lies between the floor side and the face, so the contact is
    // the face's clamped gap, as a box of half (4, 0.001) reads it, whether
    // the bottom is covered or exposed.
    let br = polygon(1, Vec2::splat(8.0), HALF);
    let bl = polygon(2, Vec2::splat(8.0), HALF);
    let cases = [
        (&br, Vec2::new(-6.0, 15.5), FACE_TOP | FACE_RIGHT),
        (&bl, Vec2::new(22.0, 15.5), FACE_TOP | FACE_LEFT),
        (
            &br,
            Vec2::new(-6.0, 15.5),
            FACE_TOP | FACE_RIGHT | FACE_BOTTOM,
        ),
        (
            &bl,
            Vec2::new(22.0, 15.5),
            FACE_TOP | FACE_LEFT | FACE_BOTTOM,
        ),
    ];
    for (poly, center, mask) in cases {
        for half in [
            Vec2::new(4.0, 0.0),
            Vec2::new(4.0, 0.001),
            Vec2::new(4.0, 1e-6),
            Vec2::new(4.0, 1e-7),
        ] {
            let contact = box_contact(center, half, poly, mask, 4.0);
            assert!(
                contact.is_some_and(|c| c.normal.abs_diff_eq(poly.normal, 1e-4)
                    && (c.penetration + 1.768).abs() < 1e-3),
                "half {half} at {center}, mask {mask:#06b}: {contact:?}"
            );
        }
    }
}

#[test]
fn a_wide_cells_slope_face_is_never_read_as_a_side() {
    // A 2:1 low cell 64 x 16 has a face normal of (-0.124, -0.992), past the
    // 0.99 an axis side's test once took, so its face read as the top and
    // the outline's mask 0 covered it. Each kind on cells up to 1:16 and
    // 16:1 keeps its face a face, and a box overlapping the wide face meets
    // it under mask 0.
    for code in 1..=12u8 {
        for (w, h) in [(16.0, 16.0), (64.0, 16.0), (256.0, 16.0), (16.0, 256.0)] {
            let poly = polygon(code, Vec2::ZERO, Vec2::new(w, h) * 0.5);
            assert_eq!(
                face_of(poly.normal),
                0,
                "kind {code} on {w} x {h}: {}",
                poly.normal
            );
        }
    }
    let wide = polygon(5, Vec2::new(32.0, 8.0), Vec2::new(32.0, 8.0));
    let contact = box_contact(Vec2::new(32.0, 9.0), Vec2::splat(4.0), &wide, 0, 1.0);
    assert!(
        contact.is_some_and(|c| c.normal.abs_diff_eq(wide.normal, 1e-6)),
        "{contact:?}"
    );
}

#[test]
fn a_contact_on_a_vertex_between_exposed_sides_has_a_unit_normal() {
    // A box whose corner sits on a ramp's apex meets the sum's vertex at its
    // centre, at distance 0, wherever the slope side's separation rounds
    // just above 0; a circle centred on a vertex always does. The normal is
    // the two exposed sides' bisector there, never 0 / 0.
    let unit = |normal: Vec2| (normal.length() - 1.0).abs() < 1e-5;
    let mut bad = Vec::new();
    for (code, half) in [(1u8, 8.0f32), (9, 8.0), (1, 6.0), (9, 6.0)] {
        let half = Vec2::splat(half);
        for row in 1..10 {
            for col in 0..16 {
                let corner = Vec2::new(col as f32, row as f32) * 16.0;
                let poly = polygon(code, corner + HALF, HALF);
                let apex = corner + Vec2::new(16.0, 0.0);
                for mask in [FACE_TOP | FACE_LEFT, FACE_TOP, FACE_ALL] {
                    let contact = box_contact(apex - half, half, &poly, mask, 1.0);
                    if contact.is_some_and(|c| !unit(c.normal)) {
                        bad.push(format!("box of half {half} on kind {code}'s apex {apex}, mask {mask:#06b}: {contact:?}"));
                    }
                }
            }
        }
    }
    for code in 1..=12u8 {
        for row in 0..10 {
            for col in 0..16 {
                let corner = Vec2::new(col as f32, row as f32) * 16.0;
                let poly = polygon(code, corner + HALF, HALF);
                for &vertex in &poly.points[..poly.len] {
                    let contact = circle_contact(vertex, 3.0, &poly, FACE_ALL, 0.0);
                    if !contact.is_some_and(|c| unit(c.normal)) {
                        bad.push(format!(
                            "circle on kind {code}'s vertex {vertex}: {contact:?}"
                        ));
                    }
                }
            }
        }
    }
    assert!(
        bad.is_empty(),
        "{} contacts: {:?}",
        bad.len(),
        &bad[..bad.len().min(3)]
    );
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
            match box_sweep(from, to, half, &poly, FACE_ALL) {
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

/// The box sweep as it was before a masked side's tie: each side its own,
/// the mask left to the safety net.
fn untied_box_sweep(from: Vec2, to: Vec2, half: Vec2, poly: &SlopePolygon) -> Option<(f32, Vec2)> {
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

/// The clip as it was before a masked side's tie.
fn untied_clip(
    origin: Vec2,
    dir: Vec2,
    sides: &[(Vec2, Vec2)],
    closed: bool,
) -> Option<(f32, usize, Option<usize>)> {
    let (mut enter, mut leave) = (f32::NEG_INFINITY, f32::INFINITY);
    let (mut first, mut tie) = (None, None);
    for (k, &(point, normal)) in sides.iter().enumerate() {
        let start = (origin - point).dot(normal);
        let rate = dir.dot(normal);
        if rate.abs() < f32::EPSILON {
            if start > 0.0 || (!closed && start == 0.0) {
                return None;
            }
            continue;
        }
        let t = -start / rate;
        if rate < 0.0 {
            if t > enter {
                (enter, first, tie) = (t, Some(k), None);
            } else if t == enter {
                tie = Some(k);
            }
        } else {
            leave = leave.min(t);
        }
    }
    let crosses = if closed {
        enter <= leave
    } else {
        enter < leave
    };
    if !crosses || leave < 0.0 || (!closed && leave == 0.0) {
        return None;
    }
    Some((enter, first?, tie))
}

/// A hit's bits, so a comparison tells -0.0 from 0.0.
fn bits(hit: Option<(f32, Vec2)>) -> Option<(u32, u32, u32)> {
    hit.map(|(t, normal)| (t.to_bits(), normal.x.to_bits(), normal.y.to_bits()))
}

#[test]
fn the_existing_slope_oracles_hold() {
    // The masked side's tie turns a masked entry into a hit through an
    // exposed side within a rounding, and nothing else: with every side
    // exposed a sweep returns its bits from before, and under any mask a
    // box cast or a ray that hit before hits alike. The sweep's oracle
    // above runs at `FACE_ALL`; the queries' march and graze oracles
    // (`casts_meet_slopes_as_a_refined_march_does`,
    // `casts_graze_slopes_exactly`) are unchanged.
    let (mut swept, mut cast_hits, mut ray_hits, mut tied) = (0, 0, 0, 0);
    for code in 1..=12u8 {
        let poly = polygon(code, Vec2::ZERO, HALF);
        let mut rng = Pcg32::seeded(0x5EE9_2000 + u64::from(code));
        for probe in 0..2_000 {
            let half = Vec2::new(rng.next_range(0.5, 6.0), rng.next_range(0.5, 6.0));
            let from = Vec2::new(rng.next_range(-30.0, 30.0), rng.next_range(-30.0, 30.0));
            let to = Vec2::new(rng.next_range(-30.0, 30.0), rng.next_range(-30.0, 30.0));
            let mask = (rng.next_u32() & u32::from(FACE_ALL)) as u8;
            let at =
                format!("kind {code}, probe {probe}: half {half} from {from} to {to}, mask {mask}");
            let sweep = box_sweep(from, to, half, &poly, FACE_ALL);
            assert_eq!(
                bits(sweep),
                bits(untied_box_sweep(from, to, half, &poly)),
                "{at}"
            );
            swept += usize::from(sweep.is_some());
            let Some(dir) = (to - from).try_normalize() else {
                continue;
            };
            let (vertices, normals) = poly.minkowski(half);
            let sides: [(Vec2, Vec2); 5] = std::array::from_fn(|k| (vertices[k], normals[k]));
            let before = untied_clip(from, dir, &sides, false)
                .filter(|&(t, ..)| t >= 0.0)
                .and_then(|(t, first, tie)| Some((t, entered_normal(&normals, first, tie, mask)?)));
            let now = cast(Shape::Aabb { half_extents: half }, from, dir, &poly, mask);
            match (before, now) {
                (Some(_), _) => {
                    assert_eq!(bits(now), bits(before), "{at}: cast");
                    cast_hits += 1;
                }
                (None, Some((_, normal))) => {
                    assert!(exposed(normal, mask), "{at}: cast tied through {normal}");
                    tied += 1;
                }
                (None, None) => {}
            }
            let points: [(Vec2, Vec2); 4] =
                std::array::from_fn(|i| (poly.points[i], poly.normals[i]));
            let inside = (0..poly.len).all(|i| (from - poly.points[i]).dot(poly.normals[i]) <= 0.0);
            let before = untied_clip(from, dir, &points[..poly.len], true)
                .filter(|_| !inside)
                .and_then(|(t, first, tie)| {
                    Some((t, entered_normal(&poly.normals, first, tie, mask)?))
                });
            let now = ray(from, dir, &poly, mask);
            match (before, now) {
                (Some(_), _) => {
                    assert_eq!(bits(now), bits(before), "{at}: ray");
                    ray_hits += 1;
                }
                (None, Some((_, normal))) => {
                    assert!(exposed(normal, mask), "{at}: ray tied through {normal}");
                    tied += 1;
                }
                (None, None) => {}
            }
        }
    }
    assert!(
        swept > 2_000 && cast_hits > 2_000 && ray_hits > 2_000,
        "{swept} sweeps, {cast_hits} casts and {ray_hits} rays met their slope"
    );
    assert!(tied < 24, "{tied} random paths tied at a masked vertex");
}

#[test]
fn a_box_cast_enters_a_slope_face_at_a_masked_vertex() {
    // Where a side the shadow rule masks (`D-229`: one the polygon touches
    // at a vertex alone) meets the face, a path through their sum's vertex
    // ties the two entries, and rounding gave the masked side the entry, so
    // a box on a floor walked 2.357 px into a ramp's foot. Paths into that
    // side through the vertex, and with the face's entry 5e-4 px before or
    // after the side's along the path, meet the face within the tie's
    // tolerance of the vertex's `t`, cast and swept, at the origin's cell
    // and at 01's far corner; at the origin a path whose face entry leads
    // by 2e-3 px, 2e-3 px into the masked side's span, starts in the solid
    // neighbour and meets nothing, as before.
    let halves = [Vec2::new(4.0, 6.0), Vec2::new(5.5, 2.25), Vec2::splat(0.75)];
    let reach = 24.0;
    let mut met = 0;
    for center in [Vec2::ZERO, Vec2::new(27_500.0, 4_500.0)] {
        for code in 1..=12u8 {
            let poly = polygon(code, center, HALF);
            let legs = legs(code);
            for half in halves {
                let shape = Shape::Aabb { half_extents: half };
                let (vertices, normals) = poly.minkowski(half);
                let face = (0..5).find(|&k| normals[k] == poly.normal).unwrap();
                // The side before the face meets it at the face's first
                // vertex, the side after at the next; each runs away from
                // the face along `away`.
                let (before, after) = ((face + 4) % 5, (face + 1) % 5);
                for (side, vertex, away) in [
                    (before, vertices[face], vertices[before] - vertices[face]),
                    (
                        after,
                        vertices[after],
                        vertices[(after + 1) % 5] - vertices[after],
                    ),
                ] {
                    let bit = face_of(normals[side]);
                    if legs.iter().any(|&(leg, ..)| leg == bit) {
                        continue;
                    }
                    let mask = FACE_ALL & !bit;
                    let dir = -normals[side];
                    let away = away.normalize();
                    // An offset `s` into the side's span puts the face's
                    // entry `s · lead` before the side's.
                    let lead = away.dot(poly.normal) / dir.dot(poly.normal);
                    let scale = (vertex.abs() + Vec2::splat(reach)).max_element();
                    let tolerance = 1e-3 + 4.0 * f32::EPSILON * scale;
                    for gap in [-5e-4, 0.0, 5e-4, 2e-3] {
                        if gap > 1e-3 && center != Vec2::ZERO {
                            continue;
                        }
                        let origin = vertex + away * (gap / lead) - dir * reach;
                        let at = format!(
                            "kind {code}, half {half}, cell at {center}, side {side}, gap {gap}"
                        );
                        let cast = cast(shape, origin, dir, &poly, mask);
                        let sweep =
                            box_sweep(origin, origin + dir * reach * 2.0, half, &poly, mask)
                                .map(|(t, normal)| (t * reach * 2.0, normal));
                        if gap > 1e-3 {
                            assert_eq!(cast, None, "{at}: cast");
                            assert_eq!(sweep, None, "{at}: sweep");
                            continue;
                        }
                        for (name, hit) in [("cast", cast), ("sweep", sweep)] {
                            let (t, normal) = hit.unwrap_or_else(|| panic!("{at}: {name} missed"));
                            assert_eq!(normal, poly.normal, "{at}: {name}");
                            assert!((t - reach).abs() <= tolerance, "{at}: {name} at {t}");
                        }
                        met += 1;
                    }
                }
            }
        }
    }
    assert!(met >= 100, "{met} paths met a masked vertex's face");
}

#[test]
fn paths_past_an_exposed_sides_line_miss() {
    // Every side of the sum bounds a path, a masked one too: a path that
    // crosses an exposed side's line outside the sum and then misses it, or
    // enters it through a masked side farther than a rounding past every
    // exposed entry, meets nothing. Round 1's path, a box of half 0.1 from
    // (-10, 10) to (0.5, 0.5), crosses a unit `45 br`'s face line at
    // (-1.6, 2.4), where clipping by the exposed sides alone would meet it.
    let poly = polygon(1, Vec2::splat(0.5), Vec2::splat(0.5));
    let mask = FACE_TOP | FACE_RIGHT;
    let shape = |half: Vec2| Shape::Aabb { half_extents: half };
    let (from, to, half) = (
        Vec2::new(-10.0, 10.0),
        Vec2::new(0.5, 0.5),
        Vec2::splat(0.1),
    );
    let dir = (to - from).normalize();
    assert_eq!(cast(shape(half), from, dir, &poly, mask), None);
    assert_eq!(box_sweep(from, to, half, &poly, mask), None);
    let mut rng = Pcg32::seeded(0x5EE9_3000);
    let (mut paths, mut masked_entries) = (0, 0);
    while paths < 2_000 {
        let half = Vec2::new(rng.next_range(0.05, 0.5), rng.next_range(0.05, 0.5));
        let from = Vec2::new(rng.next_range(-10.0, 0.0), rng.next_range(0.5, 10.0));
        let to = Vec2::new(rng.next_range(-0.5, 1.5), rng.next_range(-0.5, 1.5));
        // The path on the f32 sum, in f64: each side's entry or leave along
        // the ray, its exposed sides' crossings outside the sum.
        let (vertices, normals) = poly.minkowski(half);
        let wide = |v: Vec2| (f64::from(v.x), f64::from(v.y));
        let dot = |a: (f64, f64), b: (f64, f64)| a.0 * b.0 + a.1 * b.1;
        let (o, d) = (wide(from), wide(to - from));
        let length = dot(d, d).sqrt();
        let side = |k: usize, at: (f64, f64)| {
            let (v, n) = (wide(vertices[k]), wide(normals[k]));
            dot((at.0 - v.0, at.1 - v.1), n)
        };
        let (mut enter, mut leave, mut last, mut open) =
            (f64::NEG_INFINITY, f64::INFINITY, 0, f64::NEG_INFINITY);
        let mut outside = false;
        let mut parallel = false;
        for (k, &normal) in normals.iter().enumerate() {
            let rate = dot(d, wide(normal));
            if rate.abs() < 1e-6 {
                parallel = true;
                continue;
            }
            let t = -side(k, o) / rate;
            let crossing = (o.0 + d.0 * t, o.1 + d.1 * t);
            let shown = exposed(normal, mask);
            if shown && (0.0..=1.0).contains(&t) {
                outside |= (0..5).any(|j| side(j, crossing) > 1e-3);
            }
            if rate < 0.0 {
                if t > enter {
                    (enter, last) = (t, k);
                }
                if shown {
                    open = open.max(t);
                }
            } else {
                leave = leave.min(t);
            }
        }
        let hit = enter < leave && leave > 0.0;
        let through_masked = hit && !exposed(normals[last], mask) && enter >= 0.0;
        if parallel
            || !outside
            || (hit && !through_masked)
            || (through_masked && (enter - open) * length <= 2e-3)
            || (hit && enter < 0.0)
        {
            continue;
        }
        paths += 1;
        masked_entries += usize::from(through_masked);
        let at = format!("half {half} from {from} to {to}");
        let dir = (to - from).normalize();
        assert_eq!(
            cast(shape(half), from, dir, &poly, mask),
            None,
            "{at}: cast"
        );
        assert_eq!(box_sweep(from, to, half, &poly, mask), None, "{at}: sweep");
    }
    assert!(
        masked_entries > 500,
        "{masked_entries} of the paths entered by a masked side"
    );
}

#[test]
fn a_circle_beside_a_masked_leg_reads_no_false_overlap() {
    // ch-slope-clamp (`D-229`): a ramp's top cell, 45 br or 2:1 high br, its
    // apex (80, 16) beside a floor that covers its right leg, and a peak's
    // 45 br beside a bl at the apex (32, 16), each mask `FACE_TOP`. A circle
    // of radius 4 on the floor right of the ramp's apex, or on bl's face 1 px
    // past the peak's, clamped to the face's line and read 1.3 to 5.4 px
    // deep though it stood off; a circle cast down onto the floor at x 82
    // met the face's line square before the floor. A contact is never deeper
    // than the circle's true gap, and the cast meets nothing before the
    // floor's y 12.
    let ramp = polygon(1, Vec2::new(72.0, 24.0), HALF);
    let high = polygon(9, Vec2::new(72.0, 24.0), HALF);
    let peak = polygon(1, Vec2::new(24.0, 24.0), HALF);
    let mut bad = Vec::new();
    for (poly, center) in [
        (&ramp, Vec2::new(80.5, 11.99)),
        (&ramp, Vec2::new(82.0, 11.99)),
        (&ramp, Vec2::new(84.0, 11.99)),
        (&high, Vec2::new(82.0, 11.99)),
        (&peak, Vec2::new(35.83, 14.17)),
    ] {
        let gap = center_distance(center, poly) - 4.0;
        let contact = circle_contact(center, 4.0, poly, FACE_TOP, 1.0);
        if contact.is_some_and(|c| c.penetration > 1e-4 - gap) {
            bad.push(format!("at {center}: {contact:?}, true gap {gap}"));
        }
    }
    let shape = Shape::Circle { radius: 4.0 };
    let hit = cast(shape, Vec2::new(82.0, 0.0), Vec2::Y, &ramp, FACE_TOP);
    if hit.is_some_and(|(t, _)| t < 12.0) {
        bad.push(format!("cast down from (82, 0): {hit:?}"));
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
