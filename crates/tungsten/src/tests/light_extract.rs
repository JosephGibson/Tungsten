use super::*;
use glam::{Vec2, Vec3};
use tungsten_core::{AmbientLight, CameraState, Light, Transform, World};

fn world_with_camera() -> (World, CameraState) {
    let world = World::new();
    let camera = CameraState::new();
    (world, camera)
}

#[test]
fn extract_lights_returns_ambient_when_no_lights() {
    let (world, camera) = world_with_camera();
    let ubo = extract_lights(&world, &camera, 800.0, 600.0);
    assert_eq!(ubo.count_pad, [0, 0, 0, 0]);
    assert_eq!(ubo.ambient, [1.0, 1.0, 1.0, 1.0]);
}

#[test]
fn extract_lights_uses_ambient_resource() {
    let (mut world, camera) = world_with_camera();
    world.insert_resource(AmbientLight(Vec3::splat(0.25)));
    let ubo = extract_lights(&world, &camera, 800.0, 600.0);
    assert_eq!(ubo.ambient, [0.25, 0.25, 0.25, 1.0]);
}

#[test]
fn extract_lights_caps_at_sixteen() {
    let (mut world, camera) = world_with_camera();
    for _ in 0..32 {
        let e = world.spawn();
        world.insert(e, Transform::from_position(Vec2::ZERO));
        world.insert(e, Light::point(Vec3::ONE, 4.0));
    }
    let ubo = extract_lights(&world, &camera, 800.0, 600.0);
    assert_eq!(ubo.count_pad[0], 16);
}

#[test]
fn extract_lights_keeps_directional_under_pressure() {
    let (mut world, camera) = world_with_camera();
    let dir_e = world.spawn();
    world.insert(dir_e, Transform::from_position(Vec2::new(99999.0, 99999.0)));
    world.insert(dir_e, Light::directional(Vec3::new(0.5, 0.5, 0.5), 0.0));
    for i in 0..32 {
        let e = world.spawn();
        world.insert(e, Transform::from_position(Vec2::splat(i as f32)));
        world.insert(e, Light::point(Vec3::ONE, 4.0));
    }
    let ubo = extract_lights(&world, &camera, 800.0, 600.0);
    assert_eq!(ubo.count_pad[0], 16);
    // Directional sorted first; first packed slot has kind_tag = 1.
    assert_eq!(
        ubo.lights[0].color_intensity[3], 1.0,
        "directional retained at slot 0"
    );
}

#[test]
fn extract_lights_packs_no_light_out_of_reach() {
    // The default camera's view at 800 x 600 is (0, 0) to (800, 600).
    let (mut world, camera) = world_with_camera();
    let mut lamp = |at: Vec2, radius: f32| {
        world.spawn_with((
            Transform::from_position(at),
            Light::point(Vec3::ONE, radius),
        ));
    };
    // Forty lamps off the view's left edge whose radii end short of it.
    for i in 0..40 {
        lamp(Vec2::new(-500.0 - i as f32, 300.0), 100.0);
    }
    // Three in the view, and one off it whose radius reaches in.
    for x in [100.0, 400.0, 700.0] {
        lamp(Vec2::new(x, 300.0), 50.0);
    }
    lamp(Vec2::new(-50.0, 300.0), 80.0);
    let ubo = extract_lights(&world, &camera, 800.0, 600.0);
    assert_eq!(ubo.count_pad[0], 4, "the forty take no slot");
    let mut xs: Vec<f32> = ubo.lights[..4]
        .iter()
        .map(|light| light.position_radius[0])
        .collect();
    xs.sort_by(f32::total_cmp);
    assert_eq!(xs, [-50.0, 100.0, 400.0, 700.0]);
    assert_eq!(ubo.lights[4].position_radius, [0.0; 4], "the tail is zero");
}

#[test]
fn extract_lights_leads_with_the_directional_among_lights_out_of_reach() {
    let (mut world, camera) = world_with_camera();
    for i in 0..40 {
        world.spawn_with((
            Transform::from_position(Vec2::new(-500.0 - i as f32, 300.0)),
            Light::point(Vec3::ONE, 100.0),
        ));
    }
    world.spawn_with((
        Transform::from_position(Vec2::new(400.0, 300.0)),
        Light::point(Vec3::ONE, 50.0),
    ));
    // The directional light's transform is far from the view: it has no
    // reach to end.
    world.spawn_with((
        Transform::from_position(Vec2::splat(99999.0)),
        Light::directional(Vec3::splat(0.5), 0.0),
    ));
    let ubo = extract_lights(&world, &camera, 800.0, 600.0);
    assert_eq!(ubo.count_pad[0], 2);
    assert_eq!(ubo.lights[0].color_intensity[3], 1.0, "directional first");
    assert_eq!(ubo.lights[1].position_radius[..2], [400.0, 300.0]);
}
