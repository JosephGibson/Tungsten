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
