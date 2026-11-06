use crate::components::{Light, Transform};
use crate::ecs::World;
use crate::lighting::{AmbientLight, LIGHT_CAP, MIN_LIGHT_RADIUS, light_at, light_level, luma};
use glam::{Vec2, Vec3};

const LIT_SPRITE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/shaders/lit_sprite.wgsl"
));

#[test]
fn ambient_default_is_one() {
    assert_eq!(AmbientLight::default().0, Vec3::ONE);
}

#[test]
fn light_cap_is_sixteen() {
    assert_eq!(LIGHT_CAP, 16);
    let len: usize = LIT_SPRITE
        .split_once("array<GpuLight,")
        .and_then(|(_, rest)| rest.split_once('>'))
        .expect("lit_sprite.wgsl declares array<GpuLight, N>")
        .0
        .trim()
        .parse()
        .expect("array<GpuLight, N> has a literal N");
    assert_eq!(len, LIGHT_CAP);
}

fn lamp(world: &mut World, at: Vec2, light: Light) {
    world.spawn_with((Transform::from_position(at), light));
}

fn assert_near(actual: Vec3, expected: Vec3, what: &str) {
    assert!(
        (actual - expected).abs().max_element() <= 1e-6,
        "{what}: {actual} != {expected}"
    );
}

#[test]
fn the_light_at_a_point_sums_each_light_as_the_lit_shader_does() {
    let mut world = World::new();
    let mut point = Light::point(Vec3::new(1.0, 0.5, 0.25), 100.0);
    point.intensity = 2.0;
    lamp(&mut world, Vec2::ZERO, point);
    let full = Vec3::new(2.0, 1.0, 0.5);
    assert_near(light_at(&world, Vec2::ZERO), full, "centre");
    let fifty = full * 0.25 * 50.0 / 5000.0_f32.sqrt();
    assert_near(light_at(&world, Vec2::new(50.0, 0.0)), fifty, "50 px");
    assert_near(light_at(&world, Vec2::new(0.0, -50.0)), fifty, "50 px down");
    for past in [100.0, 100.5, 250.0, 1e6] {
        assert_eq!(
            light_at(&world, Vec2::new(past, 0.0)),
            Vec3::ZERO,
            "{past} px"
        );
    }

    // A directional light adds 1/√2 a channel everywhere.
    let mut both = World::new();
    lamp(&mut both, Vec2::ZERO, point);
    lamp(
        &mut both,
        Vec2::new(-500.0, 40.0),
        Light::directional(Vec3::ONE, 0.7),
    );
    let half = Vec3::splat(std::f32::consts::FRAC_1_SQRT_2);
    assert_near(
        light_at(&both, Vec2::new(900.0, 900.0)),
        half,
        "directional",
    );
    // Two lights add.
    assert_near(
        light_at(&both, Vec2::new(50.0, 0.0)),
        fifty + half,
        "two lights",
    );

    // Ambient adds nothing, nor does a light without a Transform.
    both.insert_resource(AmbientLight(Vec3::splat(0.3)));
    let bare = both.spawn();
    both.insert(bare, Light::point(Vec3::ONE, 1000.0));
    assert_near(
        light_at(&both, Vec2::new(50.0, 0.0)),
        fifty + half,
        "ambient, bare",
    );

    // Radii the shader clamps: finite, gone 1e-4 px out, its value at the light.
    for radius in [0.0, -5.0, 1e-6] {
        let mut tiny = World::new();
        lamp(&mut tiny, Vec2::ZERO, Light::point(Vec3::ONE, radius));
        let at = light_at(&tiny, Vec2::ZERO);
        assert!(at.is_finite(), "radius {radius}: {at}");
        assert_near(at, Vec3::ONE, "clamped radius at its light");
        for out in [MIN_LIGHT_RADIUS, 2e-4, 1.0] {
            let reading = light_at(&tiny, Vec2::new(out, 0.0));
            assert_eq!(reading, Vec3::ZERO, "radius {radius}, {out} px out");
        }
    }

    // A contribution that is not finite counts as none.
    let mut odd = World::new();
    let mut hot = Light::point(Vec3::ONE, 100.0);
    hot.intensity = f32::INFINITY;
    lamp(&mut odd, Vec2::ZERO, hot);
    lamp(&mut odd, Vec2::ZERO, Light::point(Vec3::NAN, 100.0));
    lamp(
        &mut odd,
        Vec2::ZERO,
        Light::directional(Vec3::ONE, f32::NAN),
    );
    assert_eq!(light_at(&odd, Vec2::new(10.0, 0.0)), Vec3::ZERO);
}

#[test]
fn light_level_is_the_luma_of_light_at() {
    let mut world = World::new();
    lamp(
        &mut world,
        Vec2::ZERO,
        Light::point(Vec3::new(1.0, 0.5, 0.25), 80.0),
    );
    lamp(
        &mut world,
        Vec2::ZERO,
        Light::directional(Vec3::new(0.0, 0.2, 1.0), 2.0),
    );
    for point in [Vec2::ZERO, Vec2::new(20.0, 10.0), Vec2::new(400.0, 0.0)] {
        let rgb = light_at(&world, point);
        let expected = 0.2126 * rgb.x + 0.7152 * rgb.y + 0.0722 * rgb.z;
        assert!(
            (light_level(&world, point) - expected).abs() <= 1e-6,
            "{point}"
        );
        assert_eq!(light_level(&world, point), luma(rgb));
    }
    assert_eq!(light_level(&World::new(), Vec2::ZERO), 0.0);
}

/// The lines of `lit_sprite.wgsl` that `light_from` mirrors: a change to
/// any of them changes what the shader draws, so `light_from` must follow
/// it and this list with it.
#[test]
fn the_query_follows_the_lit_shaders_light_lines() {
    let lines: Vec<&str> = LIT_SPRITE.lines().map(str::trim).collect();
    for expected in [
        // Directional: a flat normal's share of (cos, sin, 1).
        "let l_dir = normalize(vec3<f32>(l.position_radius.xy, 1.0));",
        "rgb = rgb + albedo.rgb * l.color_intensity.rgb * n_dot_l;",
        // Point: the clamped radius, the height at r / 2, the attenuation.
        "let radius = max(l.position_radius.z, 0.0001);",
        "let to_light_3d = vec3<f32>(to_light_xy, radius * 0.5);",
        "let dist_xy = length(to_light_xy);",
        "let attenuation = clamp(1.0 - dist_xy / radius, 0.0, 1.0);",
        "let l_dir = normalize(to_light_3d);",
        "let n_dot_l = max(0.0, dot(n, l_dir));",
        "rgb = rgb + albedo.rgb * l.color_intensity.rgb * n_dot_l * attenuation * attenuation;",
    ] {
        assert!(
            lines.contains(&expected),
            "lit_sprite.wgsl no longer reads `{expected}`: update `light_from` to match"
        );
    }
    assert_eq!(MIN_LIGHT_RADIUS, 0.0001);
}
