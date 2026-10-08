use super::*;
use glam::Vec2;
use tungsten_core::DebugShape;
use tungsten_core::input::KeyCode;

#[test]
fn emit_system_is_noop_when_disabled() {
    let mut world = World::new();
    world.insert_resource(DebugDraw::new());
    world.insert_resource(PhysicsDebugOverlay::default());
    let e = world.spawn();
    world.insert(e, Position(Vec2::new(4.0, 6.0)));
    world.insert(e, Collider::aabb(Vec2::splat(2.0)));

    physics_debug_emit_system(&mut world);

    assert_eq!(world.get_resource::<DebugDraw>().unwrap().len(), 0);
}

#[test]
fn emit_system_pushes_one_command_per_collider_when_enabled() {
    let mut world = World::new();
    world.insert_resource(DebugDraw::new());
    world.insert_resource(PhysicsDebugOverlay {
        enabled: true,
        ..Default::default()
    });

    let a = world.spawn();
    world.insert(a, Position(Vec2::new(0.0, 0.0)));
    world.insert(a, Collider::aabb(Vec2::splat(2.0)));

    let b = world.spawn();
    world.insert(b, Position(Vec2::new(10.0, 0.0)));
    world.insert(b, Collider::circle(3.0));

    physics_debug_emit_system(&mut world);

    let dd = world.get_resource::<DebugDraw>().unwrap();
    assert_eq!(dd.len(), 2);
}

/// An enabled overlay over one AABB body with history from (0, 0) to
/// (12, 0), and a `Time` a quarter of the way through a step.
fn interpolated_body_world() -> World {
    let mut world = World::new();
    world.insert_resource(DebugDraw::new());
    world.insert_resource(PhysicsDebugOverlay {
        enabled: true,
        ..Default::default()
    });
    let mut time = Time::new();
    time.advance_frame(time.fixed_step() * 1.25);
    world.insert_resource(time);
    let e = world.spawn();
    world.insert(e, Position(Vec2::new(12.0, 0.0)));
    world.insert(e, PrevPosition(Vec2::ZERO));
    world.insert(e, Collider::aabb(Vec2::splat(2.0)));
    world
}

fn emitted_aabb(world: &World) -> (Vec2, Vec2) {
    let dd = world.get_resource::<DebugDraw>().unwrap();
    assert_eq!(dd.len(), 1);
    match dd.commands()[0].shape {
        DebugShape::Aabb { min, max } => (min, max),
        other => panic!("expected an AABB, got {other:?}"),
    }
}

#[test]
fn outline_sits_at_the_interpolated_point_its_sprite_is_drawn_at() {
    let mut world = interpolated_body_world();
    let alpha = world.get_resource::<Time>().unwrap().alpha();
    assert!((alpha - 0.25).abs() < 1e-4, "{alpha}");

    physics_debug_emit_system(&mut world);

    let (min, max) = emitted_aabb(&world);
    let center = (min + max) * 0.5;
    assert!(
        center.distance(Vec2::new(12.0 * alpha, 0.0)) < 1e-4,
        "{center}"
    );
}

#[test]
fn outline_sits_at_position_with_interpolation_off() {
    let mut world = interpolated_body_world();
    world
        .get_resource_mut::<Time>()
        .unwrap()
        .set_interpolate(false);

    physics_debug_emit_system(&mut world);

    let (min, max) = emitted_aabb(&world);
    assert_eq!((min + max) * 0.5, Vec2::new(12.0, 0.0));
}

#[test]
fn toggle_system_flips_on_f1_action() {
    let mut world = World::new();
    let mut input = InputState::new();
    input.key_down(KeyCode::F1);
    world.insert_resource(input);
    world.insert_resource(ActionMap::default_map());
    world.insert_resource(PhysicsDebugOverlay::default());

    physics_debug_toggle_system(&mut world);

    assert!(world.get_resource::<PhysicsDebugOverlay>().unwrap().enabled);
}
