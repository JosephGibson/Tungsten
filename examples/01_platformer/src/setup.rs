use std::path::PathBuf;

use glam::{Vec2, Vec3};
use tungsten::core::{
    sync_position_to_transform, AmbientLight, AnimationRegistry, AssetRegistry, AudioCommands,
    CameraBounds, CameraController, CameraMode, Entity, Light, SoundRegistry, Tag, TilemapInstance,
    TilemapRegistry, Transform, World,
};
use tungsten::physics::{
    physics_step, BodyKind, Collider, PhysicsConfig, Position, RigidBody, Velocity,
};
use tungsten::{camera_update_system, App};

use crate::extract::{extract_sprites, extract_text};
use crate::state::{
    ActiveBlackHole, AudioState, Ball, BallHue, BallSpawnState, CurrentSprite, CycleMode,
    LightingFixture, LightingFixtureMode, OrbitLight, Player, TextDisplayState, ASSETS_LOCAL,
    ASSETS_ROOT, BALL_ANIMATION_ID, BALL_RADIUS, BALL_RESTITUTION, BALL_START_SPRITE_ID, GRAVITY_Y,
    MANIFEST_LOCAL, MANIFEST_ROOT, MAP_COLS, MAP_ROWS, PLAYER_ANIMATION_ID, PLAYER_HALF,
    PLAYER_SPAWN, PLAYER_START_SPRITE_ID, TILE,
};
use crate::systems::{
    animation_system, audio_input_system, black_hole_force_system, black_hole_lifetime_system,
    camera_zoom_input_system, damage_flash_on_ball_hit, despawn_out_of_bounds, ground_detection,
    orbit_lights_system, platformer_camera_base_zoom, player_input, rainbow_ball_hue_system,
    spawn_ball_system, spawn_black_hole_system, update_text_display,
};

type ExampleSystem = fn(&mut World);

pub(crate) const RUNTIME_SYSTEM_ORDER: &[(&str, ExampleSystem)] = &[
    ("update_text_display", update_text_display),
    ("player_input", player_input),
    ("spawn_ball_system", spawn_ball_system),
    ("spawn_black_hole_system", spawn_black_hole_system),
    ("black_hole_force_system", black_hole_force_system),
    ("audio_input_system", audio_input_system),
    ("camera_zoom_input_system", camera_zoom_input_system),
    ("animation_system", animation_system),
    ("rainbow_ball_hue_system", rainbow_ball_hue_system),
    ("physics_step", physics_step),
    ("ground_detection", ground_detection),
    ("damage_flash_on_ball_hit", damage_flash_on_ball_hit),
    ("black_hole_lifetime_system", black_hole_lifetime_system),
    ("despawn_out_of_bounds", despawn_out_of_bounds),
    ("sync_position_to_transform", sync_position_to_transform),
    ("orbit_lights_system", orbit_lights_system),
    ("platformer_camera_base_zoom", platformer_camera_base_zoom),
    ("camera_update_system", camera_update_system),
];

fn lighting_fixture_from_env() -> LightingFixtureMode {
    match std::env::var("TUNGSTEN_LIGHTING_FIXTURE")
        .ok()
        .map(|s| s.to_ascii_lowercase())
    {
        Some(v) if v == "on" => LightingFixtureMode::On,
        _ => LightingFixtureMode::Off,
    }
}

pub(crate) fn configure_app(app: &mut App) {
    enable_hot_reload(app);
    app.set_manifest_roots(vec![
        PathBuf::from(MANIFEST_ROOT),
        PathBuf::from(MANIFEST_LOCAL),
    ]);
    seed_world(app.world_mut());
    install_startup(app);
    install_runtime(app);
}

fn enable_hot_reload(app: &mut App) {
    // Watch shared and example-local assets.
    app.enable_hot_reload(
        &[PathBuf::from(ASSETS_ROOT), PathBuf::from(ASSETS_LOCAL)],
        PathBuf::from(MANIFEST_LOCAL),
    );
}

fn seed_world(world: &mut World) {
    if let Some(cfg) = world.get_resource_mut::<PhysicsConfig>() {
        cfg.gravity = Vec2::new(0.0, GRAVITY_Y);
        // One-tile cells cap dense-pile pair candidates.
        cfg.broadphase_cell_size = TILE;
    }
    world.insert_resource(TextDisplayState::default());
    world.insert_resource(BallSpawnState::default());
    world.insert_resource(ActiveBlackHole::default());

    // M29 lighting fixture: parsed once at startup; switching modes requires
    // a relaunch (matches existing `TUNGSTEN_*_FIXTURE` examples).
    let fixture_mode = lighting_fixture_from_env();
    world.insert_resource(LightingFixture { mode: fixture_mode });
    match fixture_mode {
        LightingFixtureMode::On => {
            // Dim ambient so warm/cool tints read clearly on the lit sprite.
            world.insert_resource(AmbientLight(Vec3::splat(0.15)));

            // Warm point light orbits one side of the player; intensity pulses
            // sin-style so the glow breathes without drifting in hue.
            let warm_color = Vec3::new(1.0, 0.62, 0.35);
            let mut warm = Light::point(warm_color, 5.5 * TILE);
            warm.intensity = 1.0;
            let warm_e = world.spawn();
            world.insert(warm_e, Transform::from_position(PLAYER_SPAWN));
            world.insert(warm_e, warm);
            world.insert(
                warm_e,
                OrbitLight {
                    phase: 0.0,
                    speed: 1.4,
                    radius: 1.6 * TILE,
                    cycle: CycleMode::Pulse,
                    base_color: warm_color,
                },
            );

            // Cool point light orbits the opposite side and rotates hue, so
            // the player gets a slow rainbow rim while the warm light pulses.
            let cool_seed = Vec3::new(0.4, 0.65, 1.0);
            let mut cool = Light::point(cool_seed, 5.5 * TILE);
            cool.intensity = 0.9;
            let cool_e = world.spawn();
            world.insert(cool_e, Transform::from_position(PLAYER_SPAWN));
            world.insert(cool_e, cool);
            world.insert(
                cool_e,
                OrbitLight {
                    phase: std::f32::consts::PI,
                    speed: 1.4,
                    radius: 1.6 * TILE,
                    cycle: CycleMode::Hue,
                    base_color: cool_seed,
                },
            );

            // Soft directional fill from upper-right keeps shadows from
            // crushing to black while the point lights swing.
            let mut sun = Light::directional(Vec3::splat(0.7), -std::f32::consts::FRAC_PI_4);
            sun.intensity = 0.3;
            let dir = world.spawn();
            world.insert(dir, Transform::from_position(Vec2::ZERO));
            world.insert(dir, sun);
        }
        LightingFixtureMode::Off => {
            world.insert_resource(AmbientLight(Vec3::ONE));
        }
    }

    let map = world.spawn();
    world.insert(map, TilemapInstance::new("ex10_level", Vec2::ZERO));

    // Spawn past dead-zone so camera starts visibly scrolled.
    let player = world.spawn();
    world.insert(player, Player::default());
    world.insert(player, Position(PLAYER_SPAWN));
    world.insert(player, Transform::from_position(PLAYER_SPAWN));
    world.insert(player, Velocity(Vec2::ZERO));
    world.insert(player, Collider::aabb(PLAYER_HALF));
    world.insert(player, RigidBody::dynamic().with_restitution(0.0));
    world.insert(
        player,
        tungsten::core::AnimationState::new(PLAYER_ANIMATION_ID),
    );
    world.insert(player, CurrentSprite(PLAYER_START_SPRITE_ID.into()));
    world.insert(player, Tag::new("player"));
    // M26 damage-flash: attach an empty override block and the `damage_flash`
    // material id (if registered). Default zero overlay leaves the frame
    // byte-identical to the pre-M26 baseline — the tween in `systems.rs` is
    // what actually lights it up on a collision.
    world.insert(player, tungsten::core::UniformOverrideBlock::default());
    if let Some(material_id) = world
        .get_resource::<tungsten::core::MaterialRegistry>()
        .and_then(|mr| mr.get("damage_flash"))
    {
        world.insert(player, crate::state::PlayerMaterial { material_id });
    }
    configure_platformer_camera(world, player);

    let ball_spawns: &[(f32, f32, f32)] = &[
        (6.0, 17.0, 140.0),
        (10.0, 19.0, -100.0),
        (18.0, 17.0, 180.0),
        (26.0, 19.0, -160.0),
        (34.0, 17.0, 110.0),
        (44.0, 19.0, -130.0),
        (56.0, 17.0, 150.0),
        (68.0, 19.0, -90.0),
        (76.0, 17.0, 120.0),
    ];
    for (i, &(col, row, vx)) in ball_spawns.iter().enumerate() {
        let ball = world.spawn();
        world.insert(ball, Ball);
        world.insert(ball, BallHue::from_seed(i as u32));
        world.insert(ball, Position(Vec2::new(col * TILE, row * TILE)));
        world.insert(ball, Velocity(Vec2::new(vx, 0.0)));
        world.insert(ball, Collider::circle(BALL_RADIUS));
        world.insert(
            ball,
            RigidBody {
                kind: BodyKind::Dynamic,
                inv_mass: 1.0,
                restitution: BALL_RESTITUTION,
            },
        );
        world.insert(ball, tungsten::core::AnimationState::new(BALL_ANIMATION_ID));
        world.insert(ball, CurrentSprite(BALL_START_SPRITE_ID.into()));
    }
}

pub(crate) fn configure_platformer_camera(world: &mut World, player: Entity) {
    let map_bounds = CameraBounds {
        min: Vec2::ZERO,
        max: Vec2::new(MAP_COLS as f32 * TILE, MAP_ROWS as f32 * TILE),
    };
    if let Some(controller) = world.get_resource_mut::<CameraController>() {
        controller.mode = CameraMode::Follow(player);
        controller.dead_zone_size = Vec2::ZERO;
        controller.smoothing_factor = 1.0;
        controller.bounds = Some(map_bounds);
        controller.zoom_multiplier = 1.0;
        controller.shake_amplitude = Vec2::ZERO;
        controller.shake_frequency_hz = 0.0;
        controller.shake_phase = 0.0;
    }
}

fn install_startup(app: &mut App) {
    app.on_startup(|world, _renderer| {
        // D-052: manifests loaded before startup; wire asset-dependent state.
        let registry = world.get_resource::<AssetRegistry>().unwrap();
        for id in [
            "ex10_ground",
            "ex10_ground_1",
            "ex10_ground_2",
            "ex10_ground_3",
            "ex10_platform",
            "ex10_platform_1",
            "ex10_platform_2",
            "ex10_stone_wall",
            "ex10_stone_platform",
            "ex10_sky",
            "ex10_sky_1",
            "ex10_sky_2",
            "ex10_cloud_0",
            "ex10_cloud_1",
            "ex10_mountain_0",
            "ex10_mountain_1",
            "ex10_lantern",
            "ex10_vines",
            BALL_START_SPRITE_ID,
            "ex10_ball_1",
            "ex10_ball_2",
            "ex10_ball_3",
            "ex10_cursor",
            "ex10_spark",
            PLAYER_START_SPRITE_ID,
        ] {
            assert!(registry.get_sprite(id).is_some(), "missing sprite '{id}'");
        }
        let animations = world.get_resource::<AnimationRegistry>().unwrap();
        for id in [PLAYER_ANIMATION_ID, BALL_ANIMATION_ID] {
            assert!(animations.get(id).is_some(), "missing animation '{id}'");
        }
        let tilemaps = world.get_resource::<TilemapRegistry>().unwrap();
        assert!(
            tilemaps.get("ex10_level").is_some(),
            "missing tilemap 'ex10_level'"
        );

        let (
            sfx_handle,
            black_hole_sfx_handle,
            music_handle,
            sfx_volume,
            black_hole_sfx_volume,
            music_volume,
        ) = {
            let reg = world
                .get_resource::<SoundRegistry>()
                .expect("SoundRegistry missing");
            let sfx = reg.get_by_id("sfx_blip").expect("sfx_blip not found");
            let black_hole_sfx = reg
                .get_by_id("ex10_black_hole_sfx")
                .expect("ex10_black_hole_sfx not found");
            let music = reg.get_by_id("music_main").expect("music_main not found");
            (
                sfx,
                black_hole_sfx,
                music,
                reg.get_volume(sfx),
                reg.get_volume(black_hole_sfx),
                reg.get_volume(music),
            )
        };
        world.insert_resource(AudioState {
            sfx_handle,
            black_hole_sfx_handle,
            music_handle,
            sfx_volume,
            black_hole_sfx_volume,
            music_volume,
            music_playing: false,
            master_volume: 0.5,
        });
        if let Some(cmds) = world.get_resource_mut::<AudioCommands>() {
            cmds.set_master_volume(0.5);
        }
    });
}

fn install_runtime(app: &mut App) {
    // Order: text cache -> input/gameplay -> physics -> sync -> camera.
    for (name, system) in RUNTIME_SYSTEM_ORDER {
        app.add_system_named(*name, *system);
    }
    app.set_extract_sprites(extract_sprites);
    app.set_extract_text(extract_text);
}
