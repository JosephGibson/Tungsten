use std::path::PathBuf;

use glam::Vec2;
use tungsten::asset_loader;
use tungsten::core::{
    sync_position_to_transform, AssetRegistry, AudioCommands, CameraBounds, CameraController,
    CameraMode, Entity, ResolvedManifest, SoundRegistry, Tag, TilemapInstance, TilemapRegistry,
    Transform, World,
};
use tungsten::physics::{
    physics_step, BodyKind, Collider, PhysicsConfig, Position, RigidBody, Velocity,
};
use tungsten::{camera_update_system, App};

use crate::extract::{extract_sprites, extract_text};
use crate::state::{
    ActiveBlackHole, AudioState, Ball, BallSpawnState, CurrentSprite, Player, TextDisplayState,
    ASSETS_LOCAL, ASSETS_ROOT, BALL_RADIUS, BALL_RESTITUTION, GRAVITY_Y, MANIFEST_LOCAL,
    MANIFEST_ROOT, MAP_COLS, MAP_ROWS, PLAYER_HALF, PLAYER_SPAWN, TILE,
};
use crate::systems::{
    animation_system, audio_input_system, black_hole_force_system, black_hole_lifetime_system,
    camera_zoom_input_system, despawn_out_of_bounds, ground_detection, platformer_camera_base_zoom,
    player_input, spawn_ball_system, spawn_black_hole_system, update_text_display,
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
    ("physics_step", physics_step),
    ("ground_detection", ground_detection),
    ("black_hole_lifetime_system", black_hole_lifetime_system),
    ("despawn_out_of_bounds", despawn_out_of_bounds),
    ("sync_position_to_transform", sync_position_to_transform),
    ("platformer_camera_base_zoom", platformer_camera_base_zoom),
    ("camera_update_system", camera_update_system),
];

pub(crate) fn configure_app(app: &mut App) {
    enable_hot_reload(app);
    seed_world(app.world_mut());
    install_startup(app);
    install_runtime(app);
}

fn enable_hot_reload(app: &mut App) {
    // Hot reload watches both the shared root assets dir (walk sprites,
    // animation, fonts) and the local example dir (tilemap, tile sprites).
    app.enable_hot_reload(
        &[PathBuf::from(ASSETS_ROOT), PathBuf::from(ASSETS_LOCAL)],
        PathBuf::from(MANIFEST_LOCAL),
    );
}

fn seed_world(world: &mut World) {
    if let Some(cfg) = world.get_resource_mut::<PhysicsConfig>() {
        cfg.gravity = Vec2::new(0.0, GRAVITY_Y);
        // One tile wide. Smaller cells cap per-cell proxy counts so the
        // narrow-phase inside a dense pile doesn't go quadratic in the
        // ~1000-ball stress test. At ball radius 6 a 16×16 cell holds
        // ~4 tightly-packed balls instead of ~16, cutting pair candidates
        // by roughly 16×; we pay a small 2× insert cost from balls
        // occasionally straddling two cells.
        cfg.broadphase_cell_size = 16.0;
    }
    world.insert_resource(TextDisplayState::default());
    world.insert_resource(BallSpawnState::default());
    world.insert_resource(ActiveBlackHole::default());

    // Tilemap — provides the static ground, platforms, and collision layer.
    let map = world.spawn();
    world.insert(map, TilemapInstance::new("ex10_level", Vec2::ZERO));

    // Player — spawn past the camera dead-zone (x > viewport_w/2 = 256) so
    // the camera is visibly scrolled from the first frame. At x = 320 the
    // camera starts at position 320 - 256 = 64, putting the player centred
    // on screen. Moving left scrolls the background right; moving right
    // scrolls it left (until the right edge clamp at max_x = 256).
    let player = world.spawn();
    world.insert(player, Player::default());
    world.insert(player, Position(PLAYER_SPAWN));
    world.insert(player, Transform::from_position(PLAYER_SPAWN));
    world.insert(player, Velocity(Vec2::ZERO));
    world.insert(player, Collider::aabb(PLAYER_HALF));
    world.insert(player, RigidBody::dynamic().with_restitution(0.0));
    world.insert(player, tungsten::core::AnimationState::new("walk"));
    world.insert(player, CurrentSprite("walk_0".into()));
    world.insert(player, Tag::new("player"));
    configure_platformer_camera(world, player);

    // Bouncing balls spread across the level: (col, row, initial vx).
    let ball_spawns: &[(f32, f32, f32)] = &[
        (6.0, 3.0, 70.0),
        (10.0, 5.0, -50.0),
        (15.0, 3.0, 90.0),
        (20.0, 5.0, -80.0),
        (25.0, 3.0, 55.0),
        (30.0, 5.0, -65.0),
        (35.0, 3.0, 75.0),
        (40.0, 5.0, -45.0),
    ];
    for &(col, row, vx) in ball_spawns {
        let ball = world.spawn();
        world.insert(ball, Ball);
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
    app.on_startup(|world, renderer| {
        // Root manifest: fonts (sans, sans_bold, mono), walk animation + sprites, sounds.
        let root = ResolvedManifest::load(MANIFEST_ROOT).expect("Failed to load root manifest");
        asset_loader::load_all(&root, world, renderer).expect("Failed to load root assets");

        // Local manifest: tile sprites + tilemap only. Call individual loaders rather than
        // load_all to avoid overwriting the SoundRegistry/AnimationRegistry/FontRegistry
        // that were just populated from the root manifest (those registries are replaced on
        // every load_all call).
        let local = ResolvedManifest::load(MANIFEST_LOCAL).expect("Failed to load local manifest");
        asset_loader::load_sprites(&local, world, renderer).expect("Failed to load local sprites");
        asset_loader::load_tilemaps(&local, world).expect("Failed to load local tilemaps");
        asset_loader::load_particles(&local, world).expect("Failed to load local particles");

        // Verify required assets.
        let registry = world.get_resource::<AssetRegistry>().unwrap();
        for id in [
            "ex10_ground",
            "ex10_platform",
            "ex10_sky",
            "ex10_ball",
            "ex10_cursor",
            "ex10_spark",
            "walk_0",
        ] {
            assert!(registry.get_sprite(id).is_some(), "missing sprite '{id}'");
        }
        let tilemaps = world.get_resource::<TilemapRegistry>().unwrap();
        assert!(
            tilemaps.get("ex10_level").is_some(),
            "missing tilemap 'ex10_level'"
        );

        // Resolve audio handles and stash them in a resource.
        let (sfx_handle, music_handle, sfx_volume, music_volume) = {
            let reg = world
                .get_resource::<SoundRegistry>()
                .expect("SoundRegistry missing");
            let sfx = reg.get_by_id("sfx_blip").expect("sfx_blip not found");
            let music = reg.get_by_id("music_main").expect("music_main not found");
            (sfx, music, reg.get_volume(sfx), reg.get_volume(music))
        };
        world.insert_resource(AudioState {
            sfx_handle,
            music_handle,
            sfx_volume,
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
    // Ordering: text-display-cache → gameplay input → audio → animation →
    // physics → post-physics state sync → shared camera update.
    for (name, system) in RUNTIME_SYSTEM_ORDER {
        app.add_system_named(*name, *system);
    }
    app.set_extract_sprites(extract_sprites);
    app.set_extract_text(extract_text);
}
