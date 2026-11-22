use super::*;
use glam::{Vec2, Vec3};
use std::f32::consts::{FRAC_PI_2, PI};
use tungsten_core::lighting::light_from;
use tungsten_core::{Light, Pcg32};

#[test]
fn pack_lights_zeros_unused_tail() {
    let one = pack_one_light(Vec2::new(1.0, 2.0), &Light::point(Vec3::ONE, 4.0));
    let ubo = pack_lights(&[one], Vec3::splat(0.5));
    assert_eq!(ubo.count_pad, [1, 0, 0, 0]);
    assert_eq!(ubo.ambient, [0.5, 0.5, 0.5, 1.0]);
    assert_eq!(ubo.lights[0].position_radius, [1.0, 2.0, 4.0, 0.0]);
    for slot in &ubo.lights[1..] {
        assert_eq!(slot.position_radius, [0.0; 4]);
        assert_eq!(slot.color_intensity, [0.0; 4]);
    }
}

#[test]
fn cull_to_cap_keeps_directional_first() {
    let aabb = (Vec2::ZERO, Vec2::splat(10.0));
    let entries = vec![
        (Vec2::splat(5.0), Light::point(Vec3::ONE, 1.0)),
        (Vec2::splat(100.0), Light::directional(Vec3::ONE, 0.0)),
    ];
    let out = cull_to_cap(aabb, &entries);
    assert_eq!(out.len(), 2);
    // Directional always sorts first regardless of distance.
    assert_eq!(out[0].color_intensity[3], 1.0, "directional first");
    assert_eq!(out[1].color_intensity[3], 0.0, "point second");
}

#[test]
fn cull_to_cap_truncates_to_sixteen() {
    let aabb = (Vec2::ZERO, Vec2::splat(1.0));
    let mut entries: Vec<(Vec2, Light)> = Vec::new();
    for i in 0..32 {
        // Each reaches the view from where it stands.
        let at = i as f32 * 10.0;
        entries.push((Vec2::splat(at), Light::point(Vec3::ONE, at * 2.0 + 1.0)));
    }
    let out = cull_to_cap(aabb, &entries);
    assert_eq!(out.len(), LIT_LIGHT_CAP);
}

#[test]
fn past_the_cap_lights_in_view_rank_by_the_views_centre() {
    let aabb = (Vec2::ZERO, Vec2::splat(100.0));
    // Sixteen lights inside the view near its corner come first in input
    // order, then the light at its centre, as a player's lantern can.
    let mut entries: Vec<(Vec2, Light)> = (0..LIT_LIGHT_CAP)
        .map(|i| (Vec2::new(1.0 + i as f32, 1.0), Light::point(Vec3::ONE, 5.0)))
        .collect();
    entries.push((
        Vec2::splat(50.0),
        Light::point(Vec3::new(1.0, 0.0, 0.0), 5.0),
    ));
    let out = cull_to_cap(aabb, &entries);
    assert_eq!(out.len(), LIT_LIGHT_CAP);
    assert!(
        out.iter().any(|l| l.position_radius[..2] == [50.0, 50.0]),
        "the centre light keeps its slot"
    );
    assert!(
        !out.iter().any(|l| l.position_radius[..2] == [1.0, 1.0]),
        "the corner light farthest from the centre goes"
    );
}

#[test]
fn past_the_cap_a_dark_light_takes_no_slot() {
    let aabb = (Vec2::ZERO, Vec2::splat(100.0));
    let mut dark = Light::point(Vec3::ONE, 5.0);
    dark.intensity = 0.0;
    let mut entries = vec![(Vec2::splat(50.0), dark)];
    entries.extend((0..LIT_LIGHT_CAP).map(|i| {
        (
            Vec2::new(10.0 + i as f32, 10.0),
            Light::point(Vec3::ONE, 5.0),
        )
    }));
    let out = cull_to_cap(aabb, &entries);
    assert_eq!(out.len(), LIT_LIGHT_CAP);
    assert!(out.iter().all(|l| l.color_intensity[0] > 0.0));
}

#[test]
fn at_the_cap_the_order_is_unchanged() {
    let aabb = (Vec2::ZERO, Vec2::splat(100.0));
    let entries: Vec<(Vec2, Light)> = (0..LIT_LIGHT_CAP)
        .map(|i| {
            (
                Vec2::new(90.0 - i as f32, 50.0),
                Light::point(Vec3::ONE, 5.0),
            )
        })
        .collect();
    let out = cull_to_cap(aabb, &entries);
    let xs: Vec<f32> = out.iter().map(|l| l.position_radius[0]).collect();
    let input: Vec<f32> = entries.iter().map(|(p, _)| p.x).collect();
    assert_eq!(xs, input, "in view, ties keep input order");
}

#[test]
fn distance_to_aabb_sq_zero_inside() {
    let min = Vec2::ZERO;
    let max = Vec2::splat(10.0);
    assert_eq!(distance_to_aabb_sq(Vec2::splat(5.0), min, max), 0.0);
    assert_eq!(distance_to_aabb_sq(Vec2::new(15.0, 5.0), min, max), 25.0);
}

// The reach filter (redo 7.15, G1): `lights_view`, and what the list packed
// by it draws against the list before it.

/// `cull_to_cap` as it stood before the reach filter: every light kept
/// under the cap, the dark ones dropped past it. The oracle the filtered
/// list is summed against.
fn cull_to_cap_before_reach(camera_aabb: (Vec2, Vec2), entries: &[(Vec2, Light)]) -> Vec<GpuLight> {
    let (min, max) = camera_aabb;
    let directional = |light: &Light| matches!(light.kind, LightKind::Directional { .. });
    let mut kept: Vec<&(Vec2, Light)> = entries.iter().collect();
    if kept.len() > LIT_LIGHT_CAP {
        kept.retain(|(_, light)| light.intensity != 0.0);
    }
    if kept.len() > LIT_LIGHT_CAP {
        let centre = (min + max) * 0.5;
        let rank = |(pos, light): &(Vec2, Light)| {
            (
                !directional(light),
                distance_to_aabb_sq(*pos, min, max),
                pos.distance_squared(centre),
            )
        };
        let mut order: Vec<usize> = (0..kept.len()).collect();
        order.sort_by(|&a, &b| {
            rank(kept[a])
                .partial_cmp(&rank(kept[b]))
                .unwrap_or(Ordering::Equal)
        });
        order.truncate(LIT_LIGHT_CAP);
        order.sort_unstable();
        kept = order.into_iter().map(|index| kept[index]).collect();
    }
    let mut scored: Vec<(bool, f32, GpuLight)> = kept
        .into_iter()
        .map(|(pos, light)| {
            let dist = distance_to_aabb_sq(*pos, min, max);
            (directional(light), dist, pack_one_light(*pos, light))
        })
        .collect();
    scored.sort_by(|a, b| match (a.0, b.0) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal),
    });
    scored.truncate(LIT_LIGHT_CAP);
    scored.into_iter().map(|(_, _, g)| g).collect()
}

/// What the lit shader adds for the packed `lights` at `point`, by its CPU
/// mirror `light_from`, summed in the list's order as the shader's loop
/// sums.
fn lit_sum(lights: &[GpuLight], point: Vec2) -> Vec3 {
    lights.iter().fold(Vec3::ZERO, |sum, packed| {
        let [x, y, radius, _] = packed.position_radius;
        let [red, green, blue, kind] = packed.color_intensity;
        // The packed colour is the light's colour times its intensity.
        let colour = Vec3::new(red, green, blue);
        let light = if kind == 1.0 {
            Light::directional(colour, y.atan2(x))
        } else {
            Light::point(colour, radius)
        };
        sum + light_from(&light, Vec2::new(x, y), point)
    })
}

/// Whether two packed lists draw the same at every one of `points`, bit for
/// bit.
fn draws_the_same(a: &[GpuLight], b: &[GpuLight], points: &[Vec2]) -> bool {
    points.iter().all(|&point| {
        lit_sum(a, point).to_array().map(f32::to_bits)
            == lit_sum(b, point).to_array().map(f32::to_bits)
    })
}

/// A view and `count` lights about it: inside it, off its sides and corners
/// with radii that end near its edge, and far from it; some dark, some
/// dimming (a negative intensity), a few directional.
fn random_scene(rng: &mut Pcg32, count: usize) -> ((Vec2, Vec2), Vec<(Vec2, Light)>) {
    let min = Vec2::new(
        rng.next_range(-4000.0, 4000.0),
        rng.next_range(-4000.0, 4000.0),
    );
    let max = min + Vec2::new(rng.next_range(64.0, 3840.0), rng.next_range(64.0, 2160.0));
    let entries = (0..count)
        .map(|_| {
            let colour = Vec3::new(
                rng.next_f32_unit(),
                rng.next_f32_unit(),
                rng.next_f32_unit(),
            );
            // Each axis: inside the view, or off one of its two sides.
            let mut gap = Vec2::ZERO;
            let mut position = Vec2::ZERO;
            for axis in 0..2 {
                let (lo, hi) = (min[axis], max[axis]);
                match rng.next_u32() % 4 {
                    0 => {
                        gap[axis] = rng.next_range(0.5, 3000.0);
                        position[axis] = lo - gap[axis];
                    }
                    1 => {
                        gap[axis] = rng.next_range(0.5, 3000.0);
                        position[axis] = hi + gap[axis];
                    }
                    _ => position[axis] = rng.next_range(lo, hi),
                }
            }
            let away = gap.length();
            let mut light = match rng.next_u32() % 16 {
                0 => Light::directional(colour, rng.next_range(-3.0, 3.0)),
                // A radius that ends within a tenth of the view's edge.
                1..=8 if away > 0.0 => Light::point(colour, away * rng.next_range(0.9, 1.1)),
                _ => Light::point(colour, rng.next_range(1.0, 2000.0)),
            };
            light.intensity = match rng.next_u32() % 16 {
                0 | 1 => 0.0,
                2 => -rng.next_range(0.1, 2.0),
                _ => rng.next_range(0.1, 4.0),
            };
            (position, light)
        })
        .collect();
    ((min, max), entries)
}

/// Points of the view: its corners, its point nearest each light and 64
/// random ones.
fn view_points(rng: &mut Pcg32, aabb: (Vec2, Vec2), entries: &[(Vec2, Light)]) -> Vec<Vec2> {
    let (min, max) = aabb;
    let mut points = vec![min, max, Vec2::new(min.x, max.y), Vec2::new(max.x, min.y)];
    points.extend(entries.iter().map(|(position, _)| position.clamp(min, max)));
    points.extend(
        (0..64).map(|_| Vec2::new(rng.next_range(min.x, max.x), rng.next_range(min.y, max.y))),
    );
    points
}

/// The control's filter, one light too eager: `lights_view` on a point light
/// with its radius cut by a fiftieth, so it also drops a light whose reach
/// just enters the view.
fn eager_view(aabb: (Vec2, Vec2), position: Vec2, light: &Light) -> bool {
    let mut shrunk = *light;
    if let LightKind::Point { radius } = &mut shrunk.kind {
        *radius *= 0.98;
    }
    lights_view(aabb, position, &shrunk)
}

#[test]
fn lights_view_drops_only_a_light_that_lights_no_pixel() {
    let view = (Vec2::ZERO, Vec2::splat(100.0));
    let lamp = |radius: f32| Light::point(Vec3::ONE, radius);
    assert!(
        lights_view(view, Vec2::splat(50.0), &lamp(1.0)),
        "inside the view"
    );

    // 30 px off the view's left edge: by its reach.
    let off_side = Vec2::new(-30.0, 50.0);
    assert!(lights_view(view, off_side, &lamp(40.0)), "a reach into it");
    assert!(
        lights_view(view, off_side, &lamp(30.0)),
        "a reach that ends at its edge"
    );
    assert!(
        !lights_view(view, off_side, &lamp(29.0)),
        "a reach that ends short of it"
    );
    // The margin: within 1.0001 of the radius still counts as reached.
    assert!(lights_view(view, off_side, &lamp(30.0 / 1.00005)));
    assert!(!lights_view(view, off_side, &lamp(30.0 / 1.0002)));
    // Off a corner, the distance is the corner's: 50 px here.
    let off_corner = Vec2::new(-30.0, -40.0);
    assert!(lights_view(view, off_corner, &lamp(50.0)));
    assert!(!lights_view(view, off_corner, &lamp(49.0)));

    // A dark light lights nothing, wherever it stands.
    let mut dark = lamp(500.0);
    dark.intensity = 0.0;
    assert!(!lights_view(view, Vec2::splat(50.0), &dark));
    // A directional light has no reach to end, though a dark one is dark.
    let mut sun = Light::directional(Vec3::ONE, 0.3);
    assert!(lights_view(view, Vec2::splat(1.0e9), &sun));
    sun.intensity = 0.0;
    assert!(!lights_view(view, Vec2::ZERO, &sun));

    // A radius under the minimum is the minimum, as the shader clamps it.
    assert!(lights_view(view, Vec2::splat(50.0), &lamp(0.0)));
    assert!(lights_view(
        view,
        Vec2::new(-0.5 * MIN_LIGHT_RADIUS, 50.0),
        &lamp(-1.0)
    ));
    assert!(!lights_view(view, Vec2::new(-1.0, 50.0), &lamp(0.0)));

    // What is not a number is kept, and so is a radius without end.
    let far = Vec2::new(-1.0e6, 50.0);
    assert!(
        !lights_view(view, far, &lamp(1.0)),
        "the control: far and small"
    );
    assert!(lights_view(view, far, &lamp(f32::NAN)), "a NaN radius");
    assert!(lights_view(view, far, &lamp(f32::INFINITY)));
    for (colour, intensity) in [
        (Vec3::new(f32::INFINITY, 0.0, 0.0), 0.0),
        (Vec3::new(0.0, f32::NAN, 0.0), 1.0),
        (Vec3::ONE, f32::NAN),
        (Vec3::ONE, f32::INFINITY),
    ] {
        let mut wild = lamp(1.0);
        wild.color = colour;
        wild.intensity = intensity;
        assert!(
            lights_view(view, far, &wild),
            "a colour times an intensity that is not finite: {colour} x {intensity}"
        );
    }
}

#[test]
fn a_light_the_filter_drops_adds_nothing_anywhere_in_the_view() {
    let mut rng = Pcg32::seeded(0x0715_0001);
    let (mut dropped, mut dropped_lit, mut eager_caught) = (0, 0, 0);
    for scene in 0..400 {
        let (aabb, entries) = random_scene(&mut rng, LIT_LIGHT_CAP);
        let points = view_points(&mut rng, aabb, &entries);
        for (position, light) in &entries {
            if lights_view(aabb, *position, light) {
                // The control: a light the eager filter would drop that
                // does light a point of the view.
                let lights = |point: &Vec2| light_from(light, *position, *point) != Vec3::ZERO;
                if !eager_view(aabb, *position, light) && points.iter().any(lights) {
                    eager_caught += 1;
                }
                continue;
            }
            dropped += 1;
            dropped_lit += usize::from(light.intensity != 0.0);
            for point in &points {
                assert_eq!(
                    light_from(light, *position, *point),
                    Vec3::ZERO,
                    "scene {scene}: {light:?} at {position} adds to {point} of {aabb:?}"
                );
            }
        }
    }
    // Of 6,400 lights the filter drops 3,272, 2,449 of them lit lights out
    // of reach, and the eager filter is caught dropping 238 that reach the
    // view.
    assert!(dropped > 3_000, "dropped {dropped}");
    assert!(dropped_lit > 2_000, "lit lights dropped {dropped_lit}");
    assert!(eager_caught > 200, "the eager filter caught {eager_caught}");
}

#[test]
fn under_the_cap_the_filtered_list_draws_what_the_whole_list_drew() {
    let mut rng = Pcg32::seeded(0x0715_0002);
    let (mut shorter, mut eager_differs) = (0, 0);
    for scene in 0..600 {
        let count = 1 + rng.next_u32() as usize % LIT_LIGHT_CAP;
        let (aabb, entries) = random_scene(&mut rng, count);
        let points = view_points(&mut rng, aabb, &entries);
        let before = cull_to_cap_before_reach(aabb, &entries);
        let after = cull_to_cap(aabb, &entries);
        assert_eq!(before.len(), count, "under the cap the old list held all");
        shorter += usize::from(after.len() < before.len());
        assert!(
            draws_the_same(&before, &after, &points),
            "scene {scene}: {} lights against {} of {entries:?} in {aabb:?}",
            before.len(),
            after.len()
        );
        // The control: the same cull behind a filter one light too eager.
        let eager: Vec<(Vec2, Light)> = entries
            .iter()
            .filter(|(position, light)| eager_view(aabb, *position, light))
            .copied()
            .collect();
        eager_differs += usize::from(!draws_the_same(
            &before,
            &cull_to_cap(aabb, &eager),
            &points,
        ));
    }
    // 570 of the 600 scenes lose a light to the filter, and the eager
    // filter changes what 138 of them draw.
    assert!(shorter > 500, "scenes with a light dropped: {shorter}");
    assert!(
        eager_differs > 100,
        "scenes the eager filter changes: {eager_differs}"
    );
}

#[test]
fn a_filter_one_light_too_eager_changes_what_is_drawn() {
    // The control above on one scene: a lamp 10 px off the view whose reach
    // of 10.2 enters it, and a lamp in the view that does not reach that
    // side.
    let aabb = (Vec2::ZERO, Vec2::splat(100.0));
    let entries = [
        (Vec2::splat(50.0), Light::point(Vec3::ONE, 30.0)),
        (Vec2::new(-10.0, 50.0), Light::point(Vec3::ONE, 10.2)),
    ];
    let points = [Vec2::new(0.0, 50.0), Vec2::splat(50.0)];
    let before = cull_to_cap_before_reach(aabb, &entries);
    assert!(draws_the_same(
        &before,
        &cull_to_cap(aabb, &entries),
        &points
    ));
    let eager: Vec<(Vec2, Light)> = entries
        .iter()
        .filter(|(position, light)| eager_view(aabb, *position, light))
        .copied()
        .collect();
    assert_eq!(
        eager.len(),
        1,
        "the eager filter drops the lamp off the view"
    );
    assert!(!draws_the_same(
        &before,
        &cull_to_cap(aabb, &eager),
        &points
    ));
}

#[test]
fn past_the_cap_a_far_lamp_that_reaches_the_view_is_kept() {
    let aabb = (Vec2::ZERO, Vec2::splat(100.0));
    // Sixteen small lamps 10 px off the view, none reaching it, and behind
    // them a lamp 200 px off whose radius does.
    let mut entries: Vec<(Vec2, Light)> = (0..LIT_LIGHT_CAP)
        .map(|i| {
            (
                Vec2::new(-10.0, 5.0 + i as f32 * 6.0),
                Light::point(Vec3::ONE, 5.0),
            )
        })
        .collect();
    let far = Vec2::new(-200.0, 50.0);
    entries.push((far, Light::point(Vec3::new(1.0, 0.0, 0.0), 400.0)));
    let is_far = |light: &GpuLight| light.position_radius[..2] == [far.x, far.y];

    let before = cull_to_cap_before_reach(aabb, &entries);
    assert_eq!(before.len(), LIT_LIGHT_CAP);
    assert!(
        !before.iter().any(is_far),
        "the old list ranked the far lamp out behind sixteen nearer ones"
    );
    let out = cull_to_cap(aabb, &entries);
    assert_eq!(out.len(), 1, "the sixteen light no pixel of the view");
    assert!(is_far(&out[0]));
    // So the view gains the far lamp's light.
    let point = Vec2::new(0.0, 50.0);
    assert_eq!(lit_sum(&before, point), Vec3::ZERO);
    assert!(lit_sum(&out, point).x > 0.0);
}

// A light's priority past the cap (redo 7.15, G6, `D-296`).

/// Twenty lit lamps in a row through the centre of a 100 px view, each
/// within 10 px of it, and `extra` after them.
fn crowd(extra: (Vec2, Light)) -> Vec<(Vec2, Light)> {
    let mut entries: Vec<(Vec2, Light)> = (0..20)
        .map(|i| {
            (
                Vec2::new(40.0 + i as f32, 50.0),
                Light::point(Vec3::ONE, 5.0),
            )
        })
        .collect();
    entries.push(extra);
    entries
}

fn holds(out: &[GpuLight], at: Vec2) -> bool {
    out.iter()
        .any(|light| light.position_radius[..2] == [at.x, at.y])
}

#[test]
fn past_the_cap_a_priority_light_at_the_views_edge_keeps_its_slot() {
    let aabb = (Vec2::ZERO, Vec2::splat(100.0));
    let edge = Vec2::new(1.0, 1.0);
    let lantern = Light::point(Vec3::new(1.0, 0.0, 0.0), 5.0);
    let out = cull_to_cap(aabb, &crowd((edge, lantern.with_priority(1))));
    assert_eq!(out.len(), LIT_LIGHT_CAP);
    assert!(holds(&out, edge), "the priority light keeps its slot");
    // The control: at the lamps' priority it ranks last, the farthest from
    // the view's centre.
    let out = cull_to_cap(aabb, &crowd((edge, lantern)));
    assert_eq!(out.len(), LIT_LIGHT_CAP);
    assert!(
        !holds(&out, edge),
        "at priority 0 the twenty nearer the centre push it out"
    );
}

#[test]
fn a_dark_light_takes_no_slot_under_the_cap_or_past_it() {
    let aabb = (Vec2::ZERO, Vec2::splat(100.0));
    let mut dark = Light::point(Vec3::ONE, 50.0).with_priority(9);
    dark.intensity = 0.0;
    let middle = Vec2::splat(50.0);
    // Under the cap, where every light once took a slot.
    let lit = Vec2::splat(20.0);
    let out = cull_to_cap(aabb, &[(middle, dark), (lit, Light::point(Vec3::ONE, 5.0))]);
    assert_eq!(out.len(), 1);
    assert!(holds(&out, lit));
    // Past it, whatever its priority.
    let out = cull_to_cap(aabb, &crowd((middle, dark)));
    assert_eq!(out.len(), LIT_LIGHT_CAP);
    assert!(out.iter().all(|light| light.color_intensity[0] > 0.0));
}

#[test]
fn a_priority_light_out_of_reach_takes_no_slot() {
    let aabb = (Vec2::ZERO, Vec2::splat(100.0));
    let far = Vec2::new(-500.0, 50.0);
    let lantern = Light::point(Vec3::ONE, 100.0).with_priority(u8::MAX);
    assert!(
        !lights_view(aabb, far, &lantern),
        "the filter reads no priority"
    );
    assert!(cull_to_cap(aabb, &[(far, lantern)]).is_empty());
    let out = cull_to_cap(aabb, &crowd((far, lantern)));
    assert_eq!(out.len(), LIT_LIGHT_CAP);
    assert!(!holds(&out, far));
}

#[test]
fn a_priority_chooses_the_lights_packed_and_never_their_order() {
    let aabb = (Vec2::ZERO, Vec2::splat(100.0));
    let bytes = |lights: &[GpuLight]| bytemuck::cast_slice::<GpuLight, u8>(lights).to_vec();
    // Under the cap: the list is the one every priority at 0 packs.
    let mut plain: Vec<(Vec2, Light)> = (0..15)
        .map(|i| {
            let at = Vec2::new(-20.0 + i as f32 * 9.0, 130.0 - i as f32 * 7.0);
            (at, Light::point(Vec3::ONE, 80.0))
        })
        .collect();
    plain.push((Vec2::splat(9999.0), Light::directional(Vec3::ONE, 0.4)));
    let ranked: Vec<(Vec2, Light)> = plain
        .iter()
        .enumerate()
        .map(|(i, (at, light))| (*at, light.with_priority((i * 37 % 256) as u8)))
        .collect();
    let out = cull_to_cap(aabb, &plain);
    assert_eq!(out.len(), LIT_LIGHT_CAP);
    assert_eq!(bytes(&cull_to_cap(aabb, &ranked)), bytes(&out));

    // Past it: lamps off the view's left edge, the farther the higher its
    // priority. The sixteen farthest are kept, and packed nearest first.
    let lamps: Vec<(Vec2, Light)> = (0..20)
        .map(|i| {
            let gap = 10.0 + i as f32 * 5.0;
            (
                Vec2::new(-gap, 50.0),
                Light::point(Vec3::ONE, 200.0).with_priority(i as u8),
            )
        })
        .collect();
    let out = cull_to_cap(aabb, &lamps);
    let xs: Vec<f32> = out.iter().map(|light| light.position_radius[0]).collect();
    let expected: Vec<f32> = (4..20).map(|i| -(10.0 + i as f32 * 5.0)).collect();
    assert_eq!(xs, expected);
}

#[test]
fn past_the_cap_one_priority_ranks_among_itself_and_over_a_lower_directional() {
    let aabb = (Vec2::ZERO, Vec2::splat(100.0));
    // Twenty lamps at one priority: the sixteen nearest the view's centre
    // are kept, as at priority 0.
    let mut entries: Vec<(Vec2, Light)> = (0..20)
        .map(|i| {
            (
                Vec2::new(50.0 + i as f32 * 2.0, 50.0),
                Light::point(Vec3::ONE, 5.0).with_priority(7),
            )
        })
        .collect();
    let out = cull_to_cap(aabb, &entries);
    let mut xs: Vec<f32> = out.iter().map(|light| light.position_radius[0]).collect();
    xs.sort_by(f32::total_cmp);
    let nearest: Vec<f32> = (0..16).map(|i| 50.0 + i as f32 * 2.0).collect();
    assert_eq!(xs, nearest);

    // A directional light leads its own priority, not a higher one's
    // lights: under sixteen of those it keeps no slot.
    let sun = Light::directional(Vec3::splat(0.5), 0.0);
    entries.push((Vec2::ZERO, sun));
    let out = cull_to_cap(aabb, &entries);
    assert!(out.iter().all(|light| light.color_intensity[3] == 0.0));
    // At their priority it is kept, and packed first.
    let last = entries.len() - 1;
    entries[last].1 = sun.with_priority(7);
    let out = cull_to_cap(aabb, &entries);
    assert_eq!(out.len(), LIT_LIGHT_CAP);
    assert_eq!(out[0].color_intensity[3], 1.0, "directional first");
}

/// The loosest floor a backend sets a shader's `sin` and `cos`: Direct3D's
/// absolute error of 0.0008 each, past Vulkan's 2^-11.
const TRIG_FLOOR: f32 = 0.0008;

/// A quad as `lit_sprite.wgsl`'s vertex stage places it: its centre, its
/// size, and its turn's sine and cosine as that stage computed them.
#[derive(Clone, Copy)]
struct Quad {
    centre: Vec2,
    size: Vec2,
    sin_cos: (f32, f32),
}

impl Quad {
    /// A quad that is not turned.
    fn flat(centre: Vec2, size: Vec2) -> Self {
        Self {
            centre,
            size,
            sin_cos: (0.0, 1.0),
        }
    }

    /// `along` the quad's own axes, turned as the stage turns a corner.
    fn turned(&self, along: Vec2) -> Vec2 {
        let (s, c) = self.sin_cos;
        Vec2::new(along.x * c - along.y * s, along.x * s + along.y * c)
    }

    /// The quad's point at `at`, each axis -0.5..=0.5 across it: the stage's
    /// `centre + rotated` at a corner, and between the corners what a
    /// fragment reads as its position.
    fn point(&self, at: Vec2) -> Vec2 {
        self.centre + self.turned(at * self.size)
    }
}

/// The vertex stage's distance from a point light to a quad's box, line for
/// line (`the_quad_skip_follows_the_lit_shaders_lines` holds the lines).
fn quad_gap(quad: &Quad, light_at: Vec2) -> f32 {
    let (s, c) = quad.sin_cos;
    let half = quad.size.abs() * 0.5;
    let rel = light_at - quad.centre;
    let unturned = Vec2::new(rel.x * c + rel.y * s, rel.y * c - rel.x * s);
    (unturned.abs() - half).max(Vec2::ZERO).length()
}

/// The vertex stage's test of one point light, line for line: whether it
/// lies farther from the quad's box than its radius and the slack.
fn quad_marks(quad: &Quad, light_at: Vec2, light_radius: f32) -> bool {
    let centre = quad.centre;
    let half = quad.size.abs() * 0.5;
    let radius = light_radius.max(0.0001);
    let gap = quad_gap(quad, light_at);
    let rounding = (centre.x.abs() + centre.y.abs()) * 0.0001;
    let slack = rounding + (half.x + half.y + radius) * 0.004;
    gap > radius + slack
}

/// Whether the vertex stage marks at all: its sine and cosine scale the
/// quad by a `unit` within 0.3 % of 1.
fn quad_turn_holds(quad: &Quad) -> bool {
    let (s, c) = quad.sin_cos;
    let unit = c * c + s * s;
    (unit - 1.0).abs() < 0.003
}

/// The vertex stage's mark: whether a quad's vertices find the point light
/// at `light_at` out of reach of every fragment of it.
fn quad_skips(quad: &Quad, light_at: Vec2, light_radius: f32) -> bool {
    quad_turn_holds(quad) && quad_marks(quad, light_at, light_radius)
}

/// The control's mark, a fiftieth too eager and with no slack: it also marks
/// a light whose reach just enters the quad.
fn eager_skips(quad: &Quad, light_at: Vec2, light_radius: f32) -> bool {
    quad_gap(quad, light_at) > light_radius.max(0.0001) * 0.98
}

/// Whether the point light at `light_at` lights `point`, by the fragment
/// stage's CPU mirror.
fn lights(light_at: Vec2, light_radius: f32, point: Vec2) -> bool {
    light_from(&Light::point(Vec3::ONE, light_radius), light_at, point) != Vec3::ZERO
}

/// A quad and a point light about it, over the scales a game can hold:
/// centres out to 1e5, sides from a hundredth to 2e4, a quarter of them
/// mirrored, half the quads turned. Half keep the sine and cosine a CPU
/// computes; the rest move each by up to Vulkan's floor, Direct3D's, or
/// 0.002, which is past what the stage's `unit` allows. The light lies
/// inside the quad's box or off its sides and corners, most with a reach
/// that ends near the box's edge. Returns the quad, the light's position
/// and radius, and the quad's point nearest the light as [`Quad::point`]
/// takes it.
fn random_quad_and_light(rng: &mut Pcg32) -> (Quad, Vec2, f32, Vec2) {
    let scale = |rng: &mut Pcg32, lo: f32, hi: f32| 10.0_f32.powf(rng.next_range(lo, hi));
    let far = scale(rng, 0.0, 5.0);
    let centre = Vec2::new(rng.next_range(-far, far), rng.next_range(-far, far));
    let mut size = Vec2::new(scale(rng, -2.0, 4.3), scale(rng, -2.0, 4.3));
    if rng.next_u32().is_multiple_of(4) {
        size.x = -size.x;
    }
    let turn = if rng.next_u32().is_multiple_of(2) {
        0.0
    } else {
        rng.next_range(-PI, PI)
    };
    let (mut s, mut c) = turn.sin_cos();
    let off = match rng.next_u32() % 6 {
        0 => 1.0 / 2048.0,
        1 => TRIG_FLOOR,
        2 => 0.002,
        _ => 0.0,
    };
    if off > 0.0 {
        s += rng.next_range(-off, off);
        c += rng.next_range(-off, off);
    }
    let quad = Quad {
        centre,
        size,
        sin_cos: (s, c),
    };
    // The light along the quad's own axes: each inside the box, or off one
    // of its two sides.
    let half = size.abs() * 0.5;
    let mut gap = Vec2::ZERO;
    let mut along = Vec2::ZERO;
    for axis in 0..2 {
        match rng.next_u32() % 3 {
            0 => along[axis] = rng.next_range(-half[axis], half[axis]),
            side => {
                gap[axis] = scale(rng, -3.0, 4.0);
                let sign = if side == 1 { -1.0 } else { 1.0 };
                along[axis] = sign * (half[axis] + gap[axis]);
            }
        }
    }
    let away = gap.length();
    let radius = match rng.next_u32() % 8 {
        // A reach that ends within a hundredth, or a twentieth, of the box.
        0..=2 => away * rng.next_range(0.99, 1.01),
        3 | 4 => away * rng.next_range(0.95, 1.0),
        5 => away * rng.next_range(0.2, 1.0),
        _ => scale(rng, -3.0, 4.0),
    };
    let nearest = (along / size).clamp(Vec2::splat(-0.5), Vec2::splat(0.5));
    (quad, centre + quad.turned(along), radius, nearest)
}

/// Positions a fragment of `quad` can read: its corners, its point nearest
/// the light, points along its edges and inside it; and each again moved by
/// more than interpolation rounds, a fifty-thousandth of the quad's
/// coordinates and size an axis (some 170 float steps).
fn quad_points(rng: &mut Pcg32, quad: &Quad, nearest: Vec2) -> Vec<Vec2> {
    let mut at = vec![
        Vec2::new(-0.5, -0.5),
        Vec2::new(0.5, -0.5),
        Vec2::new(-0.5, 0.5),
        Vec2::new(0.5, 0.5),
        nearest,
    ];
    for _ in 0..8 {
        let along = rng.next_range(-0.5, 0.5);
        let side = if rng.next_u32().is_multiple_of(2) {
            -0.5
        } else {
            0.5
        };
        at.push(if rng.next_u32().is_multiple_of(2) {
            Vec2::new(along, side)
        } else {
            Vec2::new(side, along)
        });
    }
    at.extend((0..16).map(|_| Vec2::new(rng.next_range(-0.5, 0.5), rng.next_range(-0.5, 0.5))));
    let rounds = (quad.centre.abs().element_sum() + quad.size.abs().element_sum()) * 2e-5;
    let mut points = Vec::with_capacity(at.len() * 2);
    for at in at {
        let point = quad.point(at);
        points.push(point);
        points.push(
            point
                + Vec2::new(
                    rng.next_range(-rounds, rounds),
                    rng.next_range(-rounds, rounds),
                ),
        );
    }
    points
}

/// The lines of `lit_sprite.wgsl` that `quad_skips` and [`Quad`] mirror: a
/// change to any of them changes which lights a quad's fragments skip, so
/// the mirror and the tests on it must follow.
#[test]
fn the_quad_skip_follows_the_lit_shaders_lines() {
    let lines: Vec<&str> = crate::LIT_SPRITE_SHADER_SOURCE
        .lines()
        .map(str::trim)
        .collect();
    for expected in [
        // Where the stage puts the quad.
        "let c = cos(instance.inst_rot);",
        "let s = sin(instance.inst_rot);",
        "scaled.x * c - scaled.y * s,",
        "scaled.x * s + scaled.y * c,",
        "let centre = instance.inst_pos + instance.inst_size * 0.5;",
        "let world_pos = centre + rotated;",
        // The mark: point lights alone, by their distance from the box,
        // where the turn's own scale holds.
        "let unit = c * c + s * s;",
        "if (skip_lights && abs(unit - 1.0) < 0.003) {",
        "let half = abs(instance.inst_size) * 0.5;",
        "if (l.color_intensity.w != 1.0) {",
        "let radius = max(l.position_radius.z, 0.0001);",
        "let rel = l.position_radius.xy - centre;",
        "let unturned = vec2<f32>(rel.x * c + rel.y * s, rel.y * c - rel.x * s);",
        "let gap = length(max(abs(unturned) - half, vec2<f32>(0.0)));",
        "let rounding = (abs(centre.x) + abs(centre.y)) * 0.0001;",
        "let slack = rounding + (half.x + half.y + radius) * 0.004;",
        "if (gap > radius + slack) {",
        "skip = skip | (1u << i);",
        // A marked light takes no term.
        "if (((in.skip >> i) & 1u) == 1u) {",
    ] {
        assert!(
            lines.contains(&expected),
            "lit_sprite.wgsl no longer reads `{expected}`: update `quad_skips` to match"
        );
    }
}

#[test]
fn a_light_a_quad_skips_lights_no_fragment_of_it() {
    let mut rng = Pcg32::seeded(0x0715_0002);
    let (mut skipped, mut skipped_near, mut kept_lit) = (0, 0, 0);
    let (mut eager_lit, mut unheld) = (0, 0);
    for case in 0..32_768 {
        let (quad, light_at, radius, nearest) = random_quad_and_light(&mut rng);
        let points = quad_points(&mut rng, &quad, nearest);
        let lit = points.iter().any(|&point| lights(light_at, radius, point));
        if quad_skips(&quad, light_at, radius) {
            for &point in &points {
                assert!(
                    !lights(light_at, radius, point),
                    "case {case}: a skipped light of radius {radius} at {light_at} lights \
                     {point} of the quad at {} sized {} turned {:?}",
                    quad.centre,
                    quad.size,
                    quad.sin_cos,
                );
            }
            skipped += 1;
            // A reach that ends within a fiftieth of the nearest point.
            if light_at.distance(quad.point(nearest)) < radius * 1.02 {
                skipped_near += 1;
            }
        } else if lit {
            kept_lit += 1;
        }
        // The control: a mark a fiftieth too eager skips a light that
        // lights a fragment, so the points above can tell.
        if eager_skips(&quad, light_at, radius) && lit {
            eager_lit += 1;
        }
        if !quad_turn_holds(&quad) {
            unheld += 1;
        }
    }
    assert!(skipped > 8_000, "{skipped} lights skipped");
    assert!(skipped_near > 1_000, "{skipped_near} skipped near the edge");
    assert!(
        kept_lit > 4_000,
        "{kept_lit} lights kept that light a point"
    );
    assert!(
        eager_lit > 1_000,
        "the control skipped {eager_lit} lit lights"
    );
    assert!(unheld > 500, "{unheld} quads past the turn's bound");
}

#[test]
fn the_quad_skip_takes_the_box_and_rounds_safely() {
    // Coordinates that round: a 20,000 px quad about the origin and a light
    // 0.0028 px wide two float steps past its corner on each axis, 0.00276 px
    // off it. The corner is lit; a margin that is a share of the radius
    // alone rounds away at this size, and a slack that grows with it does
    // not.
    let quad = Quad::flat(Vec2::ZERO, Vec2::splat(20_000.0));
    let light_at = Vec2::splat(10_000.002);
    assert_eq!(light_at.x, 10_000.0 + 2.0 / 1_024.0);
    assert!(lights(light_at, 0.0028, quad.point(Vec2::splat(0.5))));
    assert!(!quad_skips(&quad, light_at, 0.0028));

    // The box, not its circle: a light of radius 0.1 beside a square of side
    // 2 is 0.3 off its edge and inside the circle through its corners; and a
    // lamp 100 px off a long strip's side with a 90 px reach is skipped,
    // though it lies well inside the strip's circle.
    let square = Quad::flat(Vec2::ZERO, Vec2::splat(2.0));
    assert!(quad_skips(&square, Vec2::new(1.3, 0.0), 0.1));
    let strip = Quad::flat(Vec2::new(4_000.0, 300.0), Vec2::new(2_000.0, 16.0));
    let beside = Vec2::new(4_000.0, 408.0);
    assert!(quad_skips(&strip, beside, 90.0));
    assert!(
        !quad_skips(&strip, beside, 100.0),
        "its reach meets the edge"
    );
    assert!(
        !quad_skips(&strip, strip.centre, 1.0),
        "a light on the quad"
    );
    // A size's sign mirrors the quad and moves no edge.
    let mirrored = Quad::flat(strip.centre, -strip.size);
    assert!(quad_skips(&mirrored, beside, 90.0));
    assert!(!quad_skips(&mirrored, beside, 100.0));

    // Turned a quarter, the strip stands: the same lamp is over its length.
    let stood = Quad {
        sin_cos: FRAC_PI_2.sin_cos(),
        ..strip
    };
    assert!(!quad_skips(&stood, beside, 90.0));
    assert!(quad_skips(&stood, Vec2::new(4_108.0, 300.0), 90.0));

    // A cosine 0.4 % long draws the strip 0.4 % longer: a lamp 5 px off
    // that longer end with a 6 px reach lights it, the test by the box
    // would mark it, and the turn's bound marks nothing.
    let long = Quad {
        sin_cos: (0.0, 1.004),
        ..strip
    };
    let end = long.point(Vec2::new(0.5, 0.0));
    assert_eq!(end, Vec2::new(5_004.0, 300.0));
    let off_the_end = end + Vec2::new(5.0, 0.0);
    assert!(lights(off_the_end, 6.0, end));
    assert!(quad_marks(&long, off_the_end, 6.0));
    assert!(!quad_skips(&long, off_the_end, 6.0));

    // The slack is small beside a reach: a lamp is kept only while it ends
    // within a few pixels of a sprite.
    let sprite = Quad::flat(Vec2::new(3_000.0, 500.0), Vec2::splat(64.0));
    let lamp = Vec2::new(3_332.0, 500.0);
    assert!(quad_skips(&sprite, lamp, 297.0), "3 px short of the sprite");
    assert!(!quad_skips(&sprite, lamp, 299.0), "a pixel short");
}
