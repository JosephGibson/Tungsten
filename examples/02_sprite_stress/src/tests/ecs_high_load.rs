use super::*;
use tungsten_core::assets::TextureHandle;

fn test_world() -> World {
    let mut world = World::new();
    world.insert_resource(DeltaTime { dt: 1.0 / 60.0 });
    world.insert_resource(CameraState::new());
    world.insert_resource(CameraController::default());
    world.insert_resource(WindowSize {
        width: 800,
        height: 600,
    });
    world.insert_resource(FrameTimings::new());
    world.insert_resource(AssetRegistry::new());
    world.insert_resource(HighLoadSteeringScratch::default());
    world
}

fn register_test_high_load_sprite(world: &mut World) {
    let registry = world
        .get_resource_mut::<AssetRegistry>()
        .expect("AssetRegistry missing");
    registry.register_sprite(
        HIGH_LOAD_SPRITE_ID.to_string(),
        FilterMode::Nearest,
        HIGH_LOAD_SPRITE_SIZE_PX,
        HIGH_LOAD_SPRITE_SIZE_PX,
        PathBuf::from(HIGH_LOAD_SPRITE_PATH),
        TextureHandle(0),
        tungsten::core::assets::UvRect::FULL,
    );
}

#[test]
fn seed_high_load_world_spawns_requested_entities_and_components() {
    let mut world = test_world();
    seed_high_load_world(&mut world, 32);

    let entities = world.query::<StressAgent>().count();
    assert_eq!(entities, 32);

    let first = world
        .query::<StressAgent>()
        .next()
        .map(|(entity, _)| entity)
        .expect("missing StressAgent entity");
    assert!(world.get::<Position>(first).is_some());
    assert!(world.get::<Velocity>(first).is_some());
    assert!(world.get::<RigidBody>(first).is_some());
    assert!(world.get::<Transform>(first).is_some());
    assert!(world.get::<Sprite>(first).is_some());
    assert!(world.get::<Visibility>(first).is_some());

    let state = world.get_resource::<HighLoadSceneState>().unwrap();
    assert_eq!(state.entity_count, 32);
    assert!(world.get_resource::<HighLoadSteeringScratch>().is_some());

    let controller = world.get_resource::<CameraController>().unwrap();
    assert!(matches!(controller.mode, CameraMode::Follow(entity) if entity == state.leader));
}

#[test]
fn steer_agents_system_changes_nearby_velocities_deterministically() {
    let mut world_a = test_world();
    world_a.insert_resource(TelemetryState::default());
    let a = world_a.spawn();
    world_a.insert(
        a,
        StressAgent {
            phase: 0.3,
            tint_seed: 0.2,
        },
    );
    world_a.insert(a, Position(Vec2::new(100.0, 100.0)));
    world_a.insert(a, Velocity(Vec2::new(80.0, 0.0)));

    let b = world_a.spawn();
    world_a.insert(
        b,
        StressAgent {
            phase: 1.1,
            tint_seed: 0.7,
        },
    );
    world_a.insert(b, Position(Vec2::new(112.0, 100.0)));
    world_a.insert(b, Velocity(Vec2::new(-80.0, 0.0)));

    let mut world_b = test_world();
    world_b.insert_resource(TelemetryState::default());
    let a2 = world_b.spawn();
    world_b.insert(
        a2,
        StressAgent {
            phase: 0.3,
            tint_seed: 0.2,
        },
    );
    world_b.insert(a2, Position(Vec2::new(100.0, 100.0)));
    world_b.insert(a2, Velocity(Vec2::new(80.0, 0.0)));

    let b2 = world_b.spawn();
    world_b.insert(
        b2,
        StressAgent {
            phase: 1.1,
            tint_seed: 0.7,
        },
    );
    world_b.insert(b2, Position(Vec2::new(112.0, 100.0)));
    world_b.insert(b2, Velocity(Vec2::new(-80.0, 0.0)));

    steer_agents_system(&mut world_a);
    steer_agents_system(&mut world_b);

    let vel_a = world_a.get::<Velocity>(a).unwrap().0;
    let vel_b = world_b.get::<Velocity>(a2).unwrap().0;
    assert_ne!(vel_a, Vec2::new(80.0, 0.0));
    assert_eq!(vel_a, vel_b);
}

#[test]
fn confine_agents_system_clamps_and_reflects() {
    let mut world = test_world();
    let entity = world.spawn();
    world.insert(entity, Position(Vec2::new(-8.0, HIGH_LOAD_WORLD_HEIGHT)));
    world.insert(entity, Velocity(Vec2::new(-40.0, 55.0)));

    confine_agents_system(&mut world);

    let position = world.get::<Position>(entity).unwrap().0;
    let velocity = world.get::<Velocity>(entity).unwrap().0;
    assert_eq!(position.x, 0.0);
    assert_eq!(position.y, HIGH_LOAD_WORLD_HEIGHT - HIGH_LOAD_SPRITE_SIZE);
    assert!(velocity.x > 0.0);
    assert!(velocity.y < 0.0);
}

#[test]
fn orient_agents_system_writes_rotation_from_velocity() {
    let mut world = test_world();
    let entity = world.spawn();
    world.insert(entity, Velocity(Vec2::new(0.0, 10.0)));
    world.insert(entity, Transform::default());

    orient_agents_system(&mut world);

    let rotation = world.get::<Transform>(entity).unwrap().rotation;
    assert!((rotation - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
}

#[test]
fn high_load_extract_culls_offscreen_agents_and_batches_visible_ones() {
    let mut world = test_world();
    register_test_high_load_sprite(&mut world);

    world.insert_resource(CameraState {
        position: Vec2::ZERO,
        zoom: 1.0,
        rotation: 0.0,
    });
    world.insert_resource(WindowSize {
        width: 100,
        height: 100,
    });

    let visible = world.spawn();
    world.insert(visible, Transform::from_position(Vec2::new(8.0, 8.0)));
    world.insert(visible, Sprite::new(HIGH_LOAD_SPRITE_ID));
    world.insert(visible, Visibility::default());

    let also_visible = world.spawn();
    world.insert(
        also_visible,
        Transform::from_position(Vec2::new(40.0, 40.0)),
    );
    world.insert(
        also_visible,
        Sprite {
            asset_id: HIGH_LOAD_SPRITE_ID.into(),
            color: [12, 34, 56, 255],
            z_order: 0,
        },
    );
    world.insert(also_visible, Visibility::default());

    let hidden = world.spawn();
    world.insert(
        hidden,
        Transform::from_position(Vec2::new(HIGH_LOAD_WORLD_WIDTH, HIGH_LOAD_WORLD_HEIGHT)),
    );
    world.insert(hidden, Sprite::new(HIGH_LOAD_SPRITE_ID));
    world.insert(hidden, Visibility::default());

    let batches = extract_high_load_sprites(&world);
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].instances.len(), 2);
}

#[test]
fn high_load_text_hud_shows_fps_and_entity_count() {
    let mut world = test_world();
    let leader = world.spawn();
    world.insert_resource(HighLoadSceneState {
        leader,
        entity_count: 42,
        elapsed: 0.0,
    });
    if let Some(timings) = world.get_resource_mut::<FrameTimings>() {
        timings.total_ms = 20.0;
    }

    let text = extract_high_load_text(&world);
    assert_eq!(text.len(), 2);
    assert!(text[1].content.contains("FPS: 50"));
    assert!(text[1].content.contains("Entities: 42"));
}
