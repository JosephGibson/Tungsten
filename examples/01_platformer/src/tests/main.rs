use glam::Vec2;
mod ball_pit;
mod burning;
mod camera;
mod hazards;
mod level;
mod player;
mod presentation;
mod spawning;
mod spells;
use tungsten::core::assets::{LayerKind, TilemapData, TilemapLayer};
use tungsten::core::{
    ActionMap, AnimationState, AudioCommand, AudioCommands, AudioHandle, Binding, CameraController,
    CameraMode, CameraState, CommandBuffer, Config, DeltaTime, EventQueue, InputState, KeyCode,
    MouseButton, TilemapInstance, TilemapRegistry, Transform, World, sync_position_to_transform,
};
use tungsten::physics::{
    Collider, CollisionEvent, PhysicsConfig, Position, RigidBody, Velocity, physics_step,
};
use tungsten::testing::Harness;
use tungsten::{App, WindowSize, camera_update_system};

use crate::setup::{RUNTIME_SYSTEM_ORDER, configure_platformer_camera};
use crate::state::{
    ActiveBlackHole, AudioState, BALL_RADIUS, BALL_SPAWN_JITTER, BLACK_HOLE_LIFETIME,
    BLACK_HOLE_RADIUS, Ball, BallHue, BallSpawnState, BlackHole, CurrentSprite, GRAVITY_Y,
    MAP_COLS, MAP_ROWS, PLAYER_HALF, PLAYER_SPAWN, Player, TEXT_UPDATE_INTERVAL, TILE,
    TextDisplayState, WORLD_BOUNDS_MAX, WORLD_BOUNDS_MIN,
};
use crate::systems::{
    black_hole_force_system, black_hole_lifetime_system, cursor_to_world, despawn_out_of_bounds,
    ground_detection, platformer_camera_base_zoom, player_input, rainbow_ball_hue_system,
    spawn_ball_system, spawn_black_hole_system, update_text_display,
};

fn seed_world() -> World {
    let mut world = World::new();
    seed(&mut world);
    world
}

/// The resources every test world starts from: a bare `World` here, the
/// engine's world in `platformer_harness`.
fn seed(world: &mut World) {
    world.insert_resource(DeltaTime { dt: 1.0 / 60.0 });
    world.insert_resource(InputState::new());
    world.insert_resource(ActionMap::default_map());
    world.insert_resource(EventQueue::<CollisionEvent>::new());
    world.insert_resource(PhysicsConfig {
        gravity: Vec2::new(0.0, GRAVITY_Y),
        ..PhysicsConfig::default()
    });
    world.insert_resource(TilemapRegistry::new());
    world.insert_resource(CameraState::new());
    world.insert_resource(CameraController::default());
    world.insert_resource(WindowSize {
        width: 480,
        height: 288,
    });
}

/// A system a harness test runs each frame, with its profiling name.
type System = (&'static str, fn(&mut World));

/// Input, physics and ground detection, in the game's order.
const PHYSICS_SYSTEMS: [System; 3] = [
    ("player_input", player_input),
    ("physics_step", physics_step),
    ("ground_detection", ground_detection),
];

/// A windowless app seeded as `seed_world` seeds a world, on the headless
/// harness: each step runs the engine's frame with `systems` in order.
fn platformer_harness(systems: &[System]) -> Harness {
    let mut app = App::new(Config::default()).expect("App::new failed");
    seed(app.world_mut());
    for &(name, system) in systems {
        app.add_system_named(name, system);
    }
    Harness::new(app)
}

/// Input to change between steps, as winit events change it between redraws.
fn input_mut(harness: &mut Harness) -> &mut InputState {
    harness
        .world_mut()
        .get_resource_mut::<InputState>()
        .unwrap()
}

/// Sets the dt of the next frames and of direct system calls made before them.
fn set_dt(harness: &mut Harness, dt: f32) {
    harness.set_dt(dt);
    harness
        .world_mut()
        .get_resource_mut::<DeltaTime>()
        .unwrap()
        .dt = dt;
}

fn solid_floor(width: u32) -> TilemapData {
    let mut tiles = vec![-1i32; (width as usize) * 2];
    for x in 0..width as usize {
        tiles[width as usize + x] = 0;
    }
    TilemapData {
        tile_width: TILE as u32,
        tile_height: TILE as u32,
        width,
        height: 2,
        tileset: vec!["ex10_ground".into()],
        layers: vec![TilemapLayer {
            name: "collision".into(),
            kind: LayerKind::Collision,
            tiles,
        }],
    }
}

fn asset_path(relative: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join(relative)
}

fn real_level_world() -> World {
    let mut world = seed_world();
    seed_level(&mut world);
    world
}

/// The authored level's map and platform colliders.
fn seed_level(world: &mut World) {
    let map = TilemapData::load(asset_path("tilemaps/level.tmj")).unwrap();
    world
        .get_resource_mut::<TilemapRegistry>()
        .unwrap()
        .insert("ex10_level".into(), map);
    let entity = world.spawn();
    world.insert(entity, TilemapInstance::new("ex10_level", Vec2::ZERO));
    crate::gameplay::spawn_platform_colliders(world);
}

fn spawn_test_player(world: &mut World, position: Vec2) -> tungsten::core::Entity {
    let entity = world.spawn();
    world.insert(entity, Player::default());
    world.insert(entity, crate::state::PlayerPresentation::default());
    world.insert(entity, Position(position));
    world.insert(entity, Transform::from_position(position));
    world.insert(entity, Velocity(Vec2::ZERO));
    world.insert(entity, Collider::aabb(PLAYER_HALF));
    world.insert(entity, RigidBody::dynamic().with_restitution(0.0));
    world.insert(
        entity,
        AnimationState::new(crate::state::PLAYER_ANIMATION_ID),
    );
    world.insert(
        entity,
        CurrentSprite(crate::state::PLAYER_START_SPRITE_ID.into()),
    );
    entity
}

fn load_presentation_assets(world: &mut World) {
    use tungsten::core::{
        AnimationData, AnimationRegistry, ParticleActive, ParticleBudget, ParticleConfig,
        ParticleConfigRegistry,
    };
    let mut animations = AnimationRegistry::new();
    for name in [
        "player_idle",
        "player_walk",
        "player_jump",
        "player_fall",
        "player_land",
        "ball_spin",
        "ball_small_spin",
        "torch_flicker",
        "waterfall_flow",
        "vines_sway",
        "fireball",
    ] {
        animations.insert(
            format!("ex10_{name}"),
            AnimationData::load(asset_path(&format!("animations/{name}.json"))).unwrap(),
        );
    }
    world.insert_resource(animations);
    let mut particles = ParticleConfigRegistry::new();
    for name in [
        "jump_puff",
        "double_jump",
        "landing_dust",
        "torch_embers",
        "waterfall_spray",
        "wind_motes",
        "black_hole",
        "ball_explosion",
        "small_ball_impact",
        "fire_trail",
        "ball_burn",
        "ball_burn_embers",
    ] {
        let path = asset_path(&format!("particles/{name}.json"));
        let config = ParticleConfig::load(&path).unwrap();
        particles.register(format!("ex10_{name}"), path, config);
    }
    world.insert_resource(particles);
    world.insert_resource(ParticleActive::default());
    world.insert_resource(ParticleBudget { global_cap: 2048 });
    world.insert_resource(CommandBuffer::new());
    world.insert_resource(crate::state::EffectSequence::default());
}

fn mock_sprite(assets: &mut tungsten::core::AssetRegistry, name: &str, handle: u32, lit: bool) {
    use tungsten::core::assets::UvRect;
    use tungsten::core::{FilterMode, TextureHandle};
    assets.register_sprite(
        name.into(),
        FilterMode::Nearest,
        64,
        64,
        std::path::PathBuf::new(),
        TextureHandle(handle),
        UvRect::FULL,
        None,
        None,
        lit.then_some(TextureHandle(handle)),
    );
}

#[test]
fn configure_app_seeds_expected_bootstrap_state() {
    let mut app = App::new(Config::default()).expect("App::new failed");
    crate::setup::configure_app(&mut app);

    let world = app.world_mut();
    let physics = world.get_resource::<PhysicsConfig>().unwrap();
    assert_eq!(physics.gravity, Vec2::new(0.0, GRAVITY_Y));
    assert_eq!(physics.broadphase_cell_size, TILE);
    assert!(world.get_resource::<TextDisplayState>().is_some());

    let player_entities: Vec<_> = world.query::<Player>().map(|(e, _)| e).collect();
    assert_eq!(player_entities.len(), 1);
    assert_eq!(world.query::<Ball>().count(), 9);
    // Seeded balls are bronze orbs: they keep their authored colours.
    assert_eq!(world.query::<BallHue>().count(), 0);
    assert_eq!(world.query::<TilemapInstance>().count(), 1);

    let player = player_entities[0];
    assert!(world.get::<AnimationState>(player).is_some());
    assert!(world.get::<CurrentSprite>(player).is_some());

    let controller = world.get_resource::<CameraController>().unwrap();
    assert!(matches!(controller.mode, CameraMode::Follow(entity) if entity == player));
}

#[test]
fn runtime_system_order_matches_expected_pipeline() {
    let names: Vec<_> = RUNTIME_SYSTEM_ORDER.iter().map(|(name, _)| *name).collect();

    assert_eq!(
        names,
        vec![
            "platformer_bindings",
            "update_text_display",
            "player_input",
            "lantern_input",
            "spawn_ball_system",
            "spawn_black_hole_system",
            "black_hole_force_system",
            "cast_fireball_system",
            "audio_input_system",
            "camera_zoom_input_system",
            "rainbow_ball_hue_system",
            "move_obstacles",
            "tick_ball_fire",
            "physics_step",
            "ground_detection",
            "small_ball_impacts",
            "hazard_contacts",
            "fireball_flight_system",
            "spread_ball_fire",
            "black_hole_extinguish_system",
            "black_hole_lifetime_system",
            "despawn_out_of_bounds",
            "player_presentation_system",
            "animation_system",
            "transient_emitter_cleanup",
            "sync_position_to_transform",
            "ball_fire_particles",
            "orbit_lights_system",
            "squash_stretch_trigger_system",
            "squash_stretch_tick_system",
            "scene_effects",
            "platformer_camera_base_zoom",
            "shake_tick_system",
            "camera_update_system",
        ]
    );
}
