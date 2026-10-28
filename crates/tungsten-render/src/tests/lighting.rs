use super::*;
use glam::{Vec2, Vec3};
use tungsten_core::Light;

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
        entries.push((Vec2::splat(i as f32 * 10.0), Light::point(Vec3::ONE, 1.0)));
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
    entries.push((Vec2::splat(50.0), Light::point(Vec3::new(1.0, 0.0, 0.0), 5.0)));
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
    entries.extend(
        (0..LIT_LIGHT_CAP).map(|i| (Vec2::new(10.0 + i as f32, 10.0), Light::point(Vec3::ONE, 5.0))),
    );
    let out = cull_to_cap(aabb, &entries);
    assert_eq!(out.len(), LIT_LIGHT_CAP);
    assert!(out.iter().all(|l| l.color_intensity[0] > 0.0));
}

#[test]
fn at_the_cap_the_order_is_unchanged() {
    let aabb = (Vec2::ZERO, Vec2::splat(100.0));
    let entries: Vec<(Vec2, Light)> = (0..LIT_LIGHT_CAP)
        .map(|i| (Vec2::new(90.0 - i as f32, 50.0), Light::point(Vec3::ONE, 5.0)))
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
