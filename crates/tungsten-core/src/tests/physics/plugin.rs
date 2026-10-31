use glam::Vec2;

use super::*;
use crate::ecs::Entity;
use crate::physics::PhysicsMaterial;

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
fn physics_plugin_registers_the_sensor_event_queue() {
    let world = physics_world();
    assert!(world.has_event::<SensorEntered>());
    assert!(world.has_event::<CollisionEvent>());
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
fn a_kinematic_rigid_body_spawns_with_a_zero_velocity_unless_one_is_given() {
    let mut world = physics_world();
    let shape = serde_json::json!({ "kind": "aabb", "half_extents": [8.0, 4.0] });
    let still = spawn_body(
        &mut world,
        Vec2::new(3.0, 4.0),
        &serde_json::json!({ "kind": "kinematic", "shape": shape }),
    );
    assert_eq!(world.get::<Velocity>(still), Some(&Velocity::new(0.0, 0.0)));
    assert_eq!(world.get::<RigidBody>(still), Some(&RigidBody::kinematic()));
    assert_eq!(world.get::<Position>(still), Some(&Position::new(3.0, 4.0)));
    assert_eq!(
        world.get::<PrevPosition>(still),
        Some(&PrevPosition(Vec2::new(3.0, 4.0)))
    );
    let moving = spawn_body(
        &mut world,
        Vec2::ZERO,
        &serde_json::json!({ "kind": "kinematic", "shape": shape, "velocity": [-5.0, 2.0] }),
    );
    assert_eq!(
        world.get::<Velocity>(moving),
        Some(&Velocity::new(-5.0, 2.0))
    );
}

#[test]
fn a_sensor_rigid_body_spawns_with_a_zero_velocity_and_accepts_restitution() {
    let mut world = physics_world();
    let sensor = spawn_body(
        &mut world,
        Vec2::ZERO,
        &serde_json::json!({
            "kind": "sensor",
            "shape": { "kind": "circle", "radius": 6.0 },
            "restitution": 0.5
        }),
    );
    assert_eq!(
        world.get::<Velocity>(sensor),
        Some(&Velocity::new(0.0, 0.0))
    );
    assert_eq!(
        world.get::<RigidBody>(sensor),
        Some(&RigidBody::sensor().with_restitution(0.5))
    );
    assert_eq!(world.get::<Collider>(sensor), Some(&Collider::circle(6.0)));
}

#[test]
fn mass_on_a_kinematic_or_sensor_rigid_body_fails_naming_mass() {
    let world = physics_world();
    for kind in ["kinematic", "sensor"] {
        let message = check_error(
            &world,
            &serde_json::json!({
                "kind": kind,
                "shape": { "kind": "circle", "radius": 4.0 },
                "mass": 1.0
            }),
        );
        assert!(message.contains("`mass`"), "{kind}: {message}");
    }
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
            serde_json::json!({ "kind": "dynamic", "shape": circle, "bounce": 0.5 }),
            "unknown field `bounce`",
        ),
        (
            serde_json::json!({ "kind": "dynamic", "shape": circle, "friction": -0.5 }),
            "`friction` must be finite and at least 0 (got -0.5)",
        ),
        (
            serde_json::json!({ "kind": "static", "shape": circle, "rolling": 1e40 }),
            "`rolling` must be finite and at least 0 (got inf)",
        ),
        (
            serde_json::json!({ "kind": "dynamic", "shape": circle, "material": 3 }),
            "invalid type: integer `3`, expected a string",
        ),
        (
            serde_json::json!({ "kind": "dynamic",
                "shape": { "kind": "circle", "radius": 4.0, "sides": 6 } }),
            "unknown field `sides`",
        ),
        (
            serde_json::json!({ "kind": "kinematic", "shape": circle, "mass": 2.0 }),
            "`mass` is for dynamic bodies only",
        ),
        (
            serde_json::json!({ "kind": "sensor", "shape": circle, "mass": 2.0 }),
            "`mass` is for dynamic bodies only",
        ),
        (
            serde_json::json!({ "kind": "ghost", "shape": circle }),
            "unknown variant `ghost`",
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

#[test]
fn layers_rigid_body_spawns_its_lists_bits_and_takes_the_defaults_without_them() {
    let mut world = physics_world();
    let shape = serde_json::json!({ "kind": "circle", "radius": 4.0 });
    let listed = spawn_body(
        &mut world,
        Vec2::ZERO,
        &serde_json::json!({ "kind": "sensor", "shape": shape, "layers": [3], "mask": [2, 0, 2] }),
    );
    let body = world.get::<RigidBody>(listed).unwrap();
    assert_eq!((body.layers, body.mask), (1 << 3, 0b101));
    assert_eq!(body.kind, crate::physics::BodyKind::Sensor);
    let plain = spawn_body(
        &mut world,
        Vec2::ZERO,
        &serde_json::json!({ "kind": "dynamic", "shape": shape }),
    );
    assert_eq!(world.get::<RigidBody>(plain), Some(&RigidBody::dynamic()));
    let blind = spawn_body(
        &mut world,
        Vec2::ZERO,
        &serde_json::json!({ "kind": "static", "shape": shape, "layers": [0, 31], "mask": [] }),
    );
    let body = world.get::<RigidBody>(blind).unwrap();
    assert_eq!((body.layers, body.mask), (1 | 1 << 31, 0));
    assert!(!body.meets(&RigidBody::dynamic()) && !RigidBody::dynamic().meets(body));
    let unplaced = spawn_body(
        &mut world,
        Vec2::ZERO,
        &serde_json::json!({ "kind": "kinematic", "shape": shape, "layers": [] }),
    );
    let body = world.get::<RigidBody>(unplaced).unwrap();
    assert_eq!((body.layers, body.mask), (0, RigidBody::ALL_LAYERS));
}

#[test]
fn layers_rigid_body_fails_on_layer_32_and_a_non_integer_at_the_fields_path() {
    let world = physics_world();
    let circle = serde_json::json!({ "kind": "circle", "radius": 4.0 });
    let cases = [
        (
            serde_json::json!({ "kind": "dynamic", "shape": circle, "layers": [32] }),
            "layers[0]",
            "a layer must be 0 to 31 (got 32)",
        ),
        (
            serde_json::json!({ "kind": "dynamic", "shape": circle, "mask": [1, 40] }),
            "mask[1]",
            "a layer must be 0 to 31 (got 40)",
        ),
        (
            serde_json::json!({ "kind": "dynamic", "shape": circle, "layers": [1.5] }),
            "layers[0]",
            "invalid type: floating point `1.5`",
        ),
        (
            serde_json::json!({ "kind": "dynamic", "shape": circle, "mask": [-1] }),
            "mask[0]",
            "invalid value: integer `-1`",
        ),
        (
            serde_json::json!({ "kind": "dynamic", "shape": circle, "mask": [true] }),
            "mask[0]",
            "invalid type: boolean `true`, expected a layer number 0 to 31 or a physics layer name",
        ),
        (
            serde_json::json!({ "kind": "dynamic", "shape": circle, "layers": 3 }),
            "layers",
            "invalid type: integer `3`, expected a sequence",
        ),
    ];
    for (value, field, expected) in cases {
        let message = check_error(&world, &value);
        assert!(
            message.starts_with(&format!("component `rigid_body`, field `{field}`: "))
                && message.contains(expected),
            "{value}: {message}"
        );
    }
}

#[test]
fn rigid_body_reads_its_gravity_scale() {
    for body in [
        RigidBody::dynamic(),
        RigidBody::r#static(),
        RigidBody::kinematic(),
        RigidBody::sensor(),
    ] {
        assert_eq!(body.gravity_scale, 1.0, "{:?}", body.kind);
    }
    let mut world = physics_world();
    let shape = serde_json::json!({ "kind": "circle", "radius": 4.0 });
    let light = spawn_body(
        &mut world,
        Vec2::ZERO,
        &serde_json::json!({ "kind": "dynamic", "shape": shape, "gravity_scale": -0.25 }),
    );
    assert_eq!(
        world.get::<RigidBody>(light),
        Some(&RigidBody::dynamic().with_gravity_scale(-0.25))
    );
    let plain = spawn_body(
        &mut world,
        Vec2::ZERO,
        &serde_json::json!({ "kind": "dynamic", "shape": shape }),
    );
    assert_eq!(world.get::<RigidBody>(plain).unwrap().gravity_scale, 1.0);
    for (value, expected) in [
        (
            serde_json::json!({ "kind": "static", "shape": shape, "gravity_scale": 2.0 }),
            "`gravity_scale` is for dynamic bodies only",
        ),
        (
            serde_json::json!({ "kind": "kinematic", "shape": shape, "gravity_scale": 0.0 }),
            "`gravity_scale` is for dynamic bodies only",
        ),
        (
            serde_json::json!({ "kind": "sensor", "shape": shape, "gravity_scale": 1.0 }),
            "`gravity_scale` is for dynamic bodies only",
        ),
        (
            serde_json::json!({ "kind": "dynamic", "shape": shape, "gravity_scale": 1e40 }),
            "`gravity_scale` must be finite (got inf)",
        ),
    ] {
        let message = check_error(&world, &value);
        assert!(message.contains(expected), "{value}: {message}");
    }
}

/// A World with the plugin's components and the layers `walls` (3) and
/// `water` (5) and the material `ice`.
fn named_world() -> World {
    let mut world = physics_world();
    world.insert_resource(PhysicsLayers::new([
        ("walls".to_owned(), 3),
        ("water".to_owned(), 5),
    ]));
    world.insert_resource(PhysicsMaterials::new([("ice".to_owned(), ICE)]));
    world
}

const ICE: PhysicsMaterial = PhysicsMaterial {
    friction: 0.05,
    restitution: 0.25,
    rolling: 0.01,
};

#[test]
fn rigid_body_takes_layer_names_and_a_material() {
    let mut world = named_world();
    let shape = serde_json::json!({ "kind": "circle", "radius": 4.0 });
    // A name's bit joins the numbers'.
    let named = spawn_body(
        &mut world,
        Vec2::ZERO,
        &serde_json::json!({ "kind": "dynamic", "shape": shape,
            "layers": ["walls", 1], "mask": ["water"] }),
    );
    let body = world.get::<RigidBody>(named).unwrap();
    assert_eq!((body.layers, body.mask), (1 << 3 | 1 << 1, 1 << 5));
    // A material's coefficients, the entry's own over them.
    let icy = spawn_body(
        &mut world,
        Vec2::ZERO,
        &serde_json::json!({ "kind": "static", "shape": shape, "material": "ice" }),
    );
    assert_eq!(
        world.get::<RigidBody>(icy),
        Some(&RigidBody::r#static().with_material(ICE))
    );
    let rough = spawn_body(
        &mut world,
        Vec2::ZERO,
        &serde_json::json!({ "kind": "dynamic", "shape": shape, "material": "ice",
            "friction": 0.9, "restitution": 0.0 }),
    );
    let body = world.get::<RigidBody>(rough).unwrap();
    assert_eq!(
        (body.friction, body.restitution, body.rolling),
        (0.9, 0.0, 0.01)
    );
    // Without a material, the entry's own; without either, 0.
    let own = spawn_body(
        &mut world,
        Vec2::ZERO,
        &serde_json::json!({ "kind": "dynamic", "shape": shape,
            "friction": 0.4, "rolling": 0.2, "restitution": 0.5 }),
    );
    assert_eq!(
        world.get::<RigidBody>(own),
        Some(
            &RigidBody::dynamic()
                .with_friction(0.4)
                .with_rolling(0.2)
                .with_restitution(0.5)
        )
    );
    let plain = spawn_body(
        &mut world,
        Vec2::ZERO,
        &serde_json::json!({ "kind": "dynamic", "shape": shape }),
    );
    assert_eq!(world.get::<RigidBody>(plain), Some(&RigidBody::dynamic()));
    // Game code reads the same bits.
    let layers = world.resource::<PhysicsLayers>();
    assert_eq!(layers.bit("walls"), Some(1 << 3));
    assert_eq!(layers.layer("water"), Some(5));
    assert_eq!(layers.bits(["walls", "water"]), Ok(1 << 3 | 1 << 5));
    assert_eq!(
        layers.bits(["walls", "lava"]),
        Err(crate::physics::UnknownLayer("lava".to_owned()))
    );
    assert_eq!(world.resource::<PhysicsMaterials>().get("ice"), Some(ICE));
}

#[test]
fn an_unknown_layer_or_material_fails_the_load_at_its_field() {
    use crate::component_registry::ComponentRegistry;

    let shape = serde_json::json!({ "kind": "circle", "radius": 4.0 });
    let cases = [
        (
            serde_json::json!({ "kind": "dynamic", "shape": shape, "layers": ["walls", "lava"] }),
            "component `rigid_body`, field `layers[1]`: unknown physics layer `lava`",
        ),
        (
            serde_json::json!({ "kind": "sensor", "shape": shape, "mask": [2, "Walls"] }),
            "component `rigid_body`, field `mask[1]`: unknown physics layer `Walls`",
        ),
        (
            serde_json::json!({ "kind": "static", "shape": shape, "material": "mud" }),
            "component `rigid_body`, field `material`: unknown physics material `mud`",
        ),
    ];
    let world = named_world();
    let registry = world.resource::<ComponentRegistry>();
    for (value, expected) in &cases {
        // The shape alone checks out: the names wait for the World.
        assert!(registry.check("rigid_body", value).is_ok(), "{value}");
        let message = registry
            .check_in("rigid_body", value, &world)
            .expect_err("must fail")
            .to_string();
        assert_eq!(message, *expected, "{value}");
        // A spawn checks against its World before it builds.
        let context = SpawnContext::new(&world, Transform::default());
        let Err(error) = registry.prepare("rigid_body", value, &context) else {
            panic!("{value} prepared");
        };
        assert_eq!(error.to_string(), *expected, "{value}");
    }
    // A World without the resources knows no name.
    let bare = physics_world();
    let message = bare
        .resource::<ComponentRegistry>()
        .check_in(
            "rigid_body",
            &serde_json::json!({ "kind": "dynamic", "shape": shape, "layers": ["walls"],
                "material": "ice" }),
            &bare,
        )
        .expect_err("must fail")
        .to_string();
    assert_eq!(
        message,
        "component `rigid_body`, field `layers[0]`: unknown physics layer `walls`"
    );
}
