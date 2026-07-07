use super::*;
use glam::{Vec2, Vec3};
use tungsten_core::Light;

#[test]
fn light_ubo_byte_size_is_544() {
    assert_eq!(LightUbo::byte_size(), 544);
    assert_eq!(std::mem::size_of::<GpuLight>(), 32);
}

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
fn distance_to_aabb_sq_zero_inside() {
    let min = Vec2::ZERO;
    let max = Vec2::splat(10.0);
    assert_eq!(distance_to_aabb_sq(Vec2::splat(5.0), min, max), 0.0);
    assert_eq!(distance_to_aabb_sq(Vec2::new(15.0, 5.0), min, max), 25.0);
}
