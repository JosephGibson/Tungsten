use glam::Vec2;

use super::*;
use crate::ecs::Entity;

const DT: f32 = 1.0 / 60.0;

/// A body at `cur` whose last step started at `prev`, drawn at the origin.
fn body_with_history(world: &mut World, prev: Vec2, cur: Vec2) -> Entity {
    world.spawn_with((
        Position(cur),
        PrevPosition(prev),
        Transform::from_position(Vec2::ZERO),
    ))
}

fn body_without_history(world: &mut World, cur: Vec2) -> Entity {
    world.spawn_with((Position(cur), Transform::from_position(Vec2::ZERO)))
}

/// A world whose clock sits `alpha` of a step past its last step.
fn world_at_alpha(alpha: f32) -> World {
    let mut world = World::new();
    let mut time = Time::new();
    time.advance_frame(DT * alpha);
    assert_eq!(time.alpha(), alpha);
    world.insert_resource(time);
    world
}

fn drawn(world: &World, entity: Entity) -> Vec2 {
    world.get::<Transform>(entity).unwrap().position
}

#[test]
fn the_snapshot_copies_position_into_prev_position() {
    let mut world = World::new();
    let body = body_with_history(&mut world, Vec2::ZERO, Vec2::new(3.0, 4.0));
    let bare = body_without_history(&mut world, Vec2::new(5.0, 6.0));
    physics_prev_snapshot(&mut world);
    assert_eq!(
        world.get::<PrevPosition>(body),
        Some(&PrevPosition(Vec2::new(3.0, 4.0)))
    );
    assert_eq!(world.get::<Position>(body), Some(&Position::new(3.0, 4.0)));
    assert!(!world.has::<PrevPosition>(bare));
    assert_eq!(world.get::<Position>(bare), Some(&Position::new(5.0, 6.0)));
}

#[test]
fn the_sync_lerps_a_body_with_history_and_copies_one_without() {
    let mut world = world_at_alpha(0.25);
    let body = body_with_history(&mut world, Vec2::new(0.0, 8.0), Vec2::new(4.0, 0.0));
    let bare = body_without_history(&mut world, Vec2::new(5.0, 6.0));
    physics_sync(&mut world);
    assert_eq!(drawn(&world, body), Vec2::new(1.0, 6.0));
    assert_eq!(drawn(&world, bare), Vec2::new(5.0, 6.0));
    assert_eq!(world.get::<Position>(body), Some(&Position::new(4.0, 0.0)));
    assert_eq!(world.get::<Position>(bare), Some(&Position::new(5.0, 6.0)));
    assert_eq!(
        world.get::<PrevPosition>(body),
        Some(&PrevPosition(Vec2::new(0.0, 8.0)))
    );
}

#[test]
fn at_alpha_zero_the_sync_draws_the_previous_position() {
    let mut world = world_at_alpha(0.0);
    let body = body_with_history(&mut world, Vec2::new(1.5, -2.5), Vec2::new(9.0, 9.0));
    physics_sync(&mut world);
    assert_eq!(drawn(&world, body), Vec2::new(1.5, -2.5));
}

#[test]
fn with_interpolation_off_or_without_time_the_sync_copies() {
    let mut off = world_at_alpha(0.25);
    off.get_resource_mut::<Time>()
        .unwrap()
        .set_interpolate(false);
    let mut timeless = World::new();
    for world in [&mut off, &mut timeless] {
        let body = body_with_history(world, Vec2::ZERO, Vec2::new(4.0, 2.0));
        physics_sync(world);
        assert_eq!(drawn(world, body), Vec2::new(4.0, 2.0));
        assert_eq!(world.get::<Position>(body), Some(&Position::new(4.0, 2.0)));
    }
}

#[test]
fn physics_plugin_alone_resolves_to_the_snapshot_the_step_and_the_sync() {
    let mut schedule = Schedule::new();
    let mut world = World::new();
    PhysicsPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    assert_eq!(
        schedule.resolved_text(),
        "startup: -\npre_update: -\nfixed_update: physics_prev_snapshot, physics_step\n\
         update: -\npost_update: physics_sync\n"
    );
}

fn physics_world() -> World {
    let mut world = World::new();
    PhysicsPlugin.build(&mut Schedule::new(), &mut world);
    world
}

/// Spawns `rigid_body: value` at `at` and returns the entity.
fn spawn_body(world: &mut World, at: Vec2, value: &serde_json::Value) -> Entity {
    use crate::component_registry::ComponentRegistry;
    use crate::ecs::CommandBuffer;

    let context = SpawnContext::new(world, Transform::from_position(at));
    let prepared = world
        .resource::<ComponentRegistry>()
        .prepare("rigid_body", value, &context)
        .unwrap();
    let mut buffer = CommandBuffer::new();
    let pending = buffer.spawn();
    prepared.insert_pending(&mut buffer, pending);
    let before: Vec<Entity> = world.query::<Entity>().collect();
    world.flush(buffer);
    world
        .query::<Entity>()
        .find(|entity| !before.contains(entity))
        .unwrap()
}

fn check_error(world: &World, value: &serde_json::Value) -> String {
    world
        .resource::<crate::component_registry::ComponentRegistry>()
        .check("rigid_body", value)
        .expect_err("must fail")
        .to_string()
}

#[test]
fn physics_plugin_registers_rigid_body_on_a_bare_world() {
    let world = physics_world();
    let registry = world.resource::<crate::component_registry::ComponentRegistry>();
    assert_eq!(registry.names().collect::<Vec<_>>(), ["rigid_body"]);
}

#[test]
fn a_dynamic_rigid_body_spawns_at_the_transform() {
    let mut world = physics_world();
    let body = spawn_body(
        &mut world,
        Vec2::new(10.0, -20.0),
        &serde_json::json!({
            "kind": "dynamic",
            "shape": { "kind": "circle", "radius": 6.0 },
            "offset": [1.0, 2.0],
            "mass": 2.0,
            "restitution": 0.25
        }),
    );
    assert_eq!(
        world.get::<Position>(body),
        Some(&Position::new(10.0, -20.0))
    );
    assert_eq!(
        world.get::<PrevPosition>(body),
        Some(&PrevPosition(Vec2::new(10.0, -20.0)))
    );
    assert_eq!(world.get::<Velocity>(body), Some(&Velocity::new(0.0, 0.0)));
    assert_eq!(
        world.get::<Collider>(body),
        Some(&Collider::circle(6.0).with_offset(Vec2::new(1.0, 2.0)))
    );
    let rigid = world.get::<RigidBody>(body).unwrap();
    assert_eq!(rigid.kind, crate::physics::BodyKind::Dynamic);
    assert_eq!(rigid.inv_mass, 0.5);
    assert_eq!(rigid.restitution, 0.25);
}

#[test]
fn a_static_rigid_body_has_no_velocity_unless_one_is_given() {
    let mut world = physics_world();
    let still = spawn_body(
        &mut world,
        Vec2::ZERO,
        &serde_json::json!({
            "kind": "static",
            "shape": { "kind": "aabb", "half_extents": [8.0, 4.0] }
        }),
    );
    assert!(!world.has::<Velocity>(still));
    assert_eq!(
        world.get::<Collider>(still),
        Some(&Collider::aabb(Vec2::new(8.0, 4.0)))
    );
    assert_eq!(world.get::<RigidBody>(still), Some(&RigidBody::r#static()));
    let moving = spawn_body(
        &mut world,
        Vec2::ZERO,
        &serde_json::json!({
            "kind": "static",
            "shape": { "kind": "aabb", "half_extents": [8.0, 4.0] },
            "velocity": [3.0, 0.0]
        }),
    );
    assert_eq!(
        world.get::<Velocity>(moving),
        Some(&Velocity::new(3.0, 0.0))
    );
}

#[test]
fn rigid_body_rejects_bad_values_naming_the_field_or_value() {
    let world = physics_world();
    let circle = serde_json::json!({ "kind": "circle", "radius": 4.0 });
    let cases = [
        (
            serde_json::json!({ "kind": "static", "shape": circle, "mass": 2.0 }),
            "`mass` is for dynamic bodies only",
        ),
        (
            serde_json::json!({ "kind": "dynamic", "shape": circle, "mass": 0.0 }),
            "`mass` must be finite and > 0 with a finite inverse (got 0)",
        ),
        (
            serde_json::json!({ "kind": "dynamic", "shape": circle, "mass": -1.0 }),
            "`mass` must be finite and > 0 with a finite inverse (got -1)",
        ),
        (
            serde_json::json!({ "kind": "dynamic", "shape": circle, "mass": 1e-40 }),
            "`mass` must be finite and > 0 with a finite inverse",
        ),
        (
            serde_json::json!({ "kind": "dynamic", "shape": { "kind": "circle", "radius": 0.0 } }),
            "`shape.radius` must be in (0, 16384] (got 0)",
        ),
        (
            serde_json::json!({ "kind": "dynamic", "shape": { "kind": "circle", "radius": 1e12 } }),
            "`shape.radius` must be in (0, 16384]",
        ),
        (
            serde_json::json!({ "kind": "dynamic",
                "shape": { "kind": "aabb", "half_extents": [4.0, -1.0] } }),
            "`shape.half_extents` must be in (0, 16384] (got [4, -1])",
        ),
        (
            serde_json::json!({ "kind": "dynamic", "shape": circle, "offset": [16385.0, 0.0] }),
            "`offset` must be within ±16384 (got [16385, 0])",
        ),
        (
            serde_json::json!({ "kind": "dynamic", "shape": circle, "restitution": 1.5 }),
            "`restitution` must be in 0..=1 (got 1.5)",
        ),
        (
            serde_json::json!({ "kind": "dynamic", "shape": circle, "velocity": [1e40, 0.0] }),
            "`velocity` must be finite (got [inf, 0])",
        ),
        (
            serde_json::json!({ "kind": "dynamic", "shape": circle, "friction": 0.5 }),
            "unknown field `friction`",
        ),
        (
            serde_json::json!({ "kind": "dynamic",
                "shape": { "kind": "circle", "radius": 4.0, "sides": 6 } }),
            "unknown field `sides`",
        ),
        (
            serde_json::json!({ "kind": "kinematic", "shape": circle }),
            "unknown variant `kinematic`",
        ),
        (
            serde_json::json!({ "kind": "dynamic", "shape": { "kind": "capsule", "radius": 4.0 } }),
            "unknown variant `capsule`",
        ),
        (
            serde_json::json!({ "kind": "dynamic" }),
            "missing field `shape`",
        ),
    ];
    for (value, expected) in cases {
        let message = check_error(&world, &value);
        assert!(
            message.starts_with("component `rigid_body`") && message.contains(expected),
            "{value}: {message}"
        );
    }
}
