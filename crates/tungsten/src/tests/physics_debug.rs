use super::*;
use glam::Vec2;
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
