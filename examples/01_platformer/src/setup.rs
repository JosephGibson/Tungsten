use std::path::PathBuf;

use glam::{Vec2, Vec3};
use tungsten::core::{
    AmbientLight, AnimationRegistry, AssetRegistry, AudioCommands, CameraBounds, CameraController,
    CameraMode, Easing, Entity, Light, ParallaxLayer, ParticleBudget, ParticleConfigRegistry,
    ParticleEmitter, ParticleEmitterState, SoundRegistry, Sprite, SpriteSquashStretch,
    SquashTrigger, Tag, TilemapInstance, TilemapRegistry, Transform, Visibility, World,
    sync_position_to_transform,
};
use tungsten::physics::{
    BodyKind, Collider, PhysicsConfig, Position, RigidBody, Velocity, physics_step,
};
use tungsten::{
    App, camera_update_system, shake_tick_system, squash_stretch_tick_system,
    squash_stretch_trigger_system,
};

use crate::extract::{extract_sprites, extract_text};
use crate::level_layout::{EMITTERS, PROPS};
use crate::state::{
    ASSETS_LOCAL, ASSETS_ROOT, ActiveBlackHole, AudioState, BALL_ANIMATION_ID, BALL_RADIUS,
    BALL_RESTITUTION, BALL_START_SPRITE_ID, Ball, BallSpawnState, CurrentSprite, CycleMode,
    EffectSounds, GRAVITY_Y, LightingFixture, LightingFixtureMode, MANIFEST_LOCAL, MANIFEST_ROOT,
    MAP_COLS, MAP_ROWS, OrbitLight, PLAYER_ANIMATION_ID, PLAYER_HALF, PLAYER_SPAWN,
    PLAYER_START_SPRITE_ID, Player, TILE, TextDisplayState,
};
use crate::state::{AmbientEmitter, AnimatedProp, EffectSequence, PlayerPresentation};
use crate::systems::{
    animation_system, audio_input_system, black_hole_extinguish_system, black_hole_force_system,
    black_hole_lifetime_system, camera_zoom_input_system, despawn_out_of_bounds, ground_detection,
    orbit_lights_system, platformer_camera_base_zoom, player_input, player_presentation_system,
    rainbow_ball_hue_system, spawn_ball_system, spawn_black_hole_system, transient_emitter_cleanup,
    update_text_display,
};

type ExampleSystem = fn(&mut World);

pub(crate) const RUNTIME_SYSTEM_ORDER: &[(&str, ExampleSystem)] = &[
    ("platformer_bindings", platformer_bindings),
    ("update_text_display", update_text_display),
    ("player_input", player_input),
    ("lantern_input", crate::gameplay::lantern_input),
    ("spawn_ball_system", spawn_ball_system),
    ("spawn_black_hole_system", spawn_black_hole_system),
    ("black_hole_force_system", black_hole_force_system),
    (
        "cast_fireball_system",
        crate::fireball::cast_fireball_system,
    ),
    ("audio_input_system", audio_input_system),
    ("camera_zoom_input_system", camera_zoom_input_system),
    ("rainbow_ball_hue_system", rainbow_ball_hue_system),
    ("move_obstacles", crate::gameplay::move_obstacles),
    ("tick_ball_fire", crate::burning::tick_ball_fire),
    ("physics_step", physics_step),
    ("ground_detection", ground_detection),
    ("small_ball_impacts", crate::gameplay::small_ball_impacts),
    ("hazard_contacts", crate::gameplay::hazard_contacts),
    (
        "fireball_flight_system",
        crate::fireball::fireball_flight_system,
    ),
    ("spread_ball_fire", crate::burning::spread_ball_fire),
    ("black_hole_extinguish_system", black_hole_extinguish_system),
    ("black_hole_lifetime_system", black_hole_lifetime_system),
    ("despawn_out_of_bounds", despawn_out_of_bounds),
    ("player_presentation_system", player_presentation_system),
    ("animation_system", animation_system),
    ("transient_emitter_cleanup", transient_emitter_cleanup),
    ("sync_position_to_transform", sync_position_to_transform),
    ("ball_fire_particles", crate::burning::ball_fire_particles),
    ("orbit_lights_system", orbit_lights_system),
    // M30: the trigger reads the current event window, so it must follow
    // `ground_detection`; `shake_tick_system` must precede the camera update.
    (
        "squash_stretch_trigger_system",
        squash_stretch_trigger_system,
    ),
    ("squash_stretch_tick_system", squash_stretch_tick_system),
    ("scene_effects", crate::gameplay::scene_effects),
    ("platformer_camera_base_zoom", platformer_camera_base_zoom),
    ("shake_tick_system", shake_tick_system),
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
    platformer_bindings(app.world_mut());
    install_startup(app);
    install_runtime(app);
}

/// Keep these example-local controls across shared input.json hot reloads.
pub(crate) fn platformer_bindings(world: &mut World) {
    use tungsten::core::{ActionMap, Binding, KeyCode, MouseButton};
    let Some(actions) = world.get_resource_mut::<ActionMap>() else {
        return;
    };
    for (name, binding) in [
        (
            "spawn_small_ball",
            Binding::Mouse {
                button: MouseButton::Middle,
            },
        ),
        (
            "audio_stop_all",
            Binding::Key {
                code: KeyCode::KeyS,
            },
        ),
        // Mouse 4 (winit `Back`).
        (
            "cast_fireball",
            Binding::Mouse {
                button: MouseButton::Other(4),
            },
        ),
    ] {
        if actions.bindings(name) != [binding] {
            actions.replace_bindings(name, vec![binding]);
        }
    }
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
    world.insert_resource(ParticleBudget { global_cap: 2048 });
    world.insert_resource(EffectSequence::default());
    // Existing stock passes: highlights bloom softly, while the play area stays
    // sharp and readable. No blur, grain or color cycling over the pixel art.
    use tungsten::core::post::{BloomParams, PostPass, PostStack, VignetteParams};
    world.insert_resource(PostStack(vec![
        PostPass::Bloom(BloomParams {
            threshold: 0.72,
            knee: 0.16,
            intensity: 0.22,
            radius: 0.65,
        }),
        PostPass::Vignette(VignetteParams {
            inner: 0.38,
            outer: 0.78,
            strength: 0.18,
            color: [0.015, 0.013, 0.02, 1.0],
        }),
    ]));

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
            world.insert_resource(AmbientLight(Vec3::new(0.62, 0.62, 0.57)));
            let moon = world.spawn();
            let mut light =
                Light::directional(Vec3::new(0.85, 0.85, 0.72), -std::f32::consts::FRAC_PI_4);
            light.intensity = 0.1;
            world.insert(moon, light);
            world.insert(moon, Transform::default());
        }
    }

    let map = world.spawn();
    world.insert(map, TilemapInstance::new("ex10_level", Vec2::ZERO));
    crate::gameplay::spawn_platform_colliders(world);

    // Safe apron; asset-dependent props are installed after manifest loading.
    let player = world.spawn();
    world.insert(player, Player::default());
    world.insert(player, crate::gameplay::Health::default());
    world.insert(player, PlayerPresentation::default());
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
    // M30 squash on landing. Not a `Tween` (`D-073`): the player's single
    // `D-055` tween slot stays with the M26 damage flash below, so both fire
    // together on a hazard hit.
    world.insert(
        player,
        SpriteSquashStretch {
            on: SquashTrigger::OnLand,
            amount: Vec2::new(1.16, 0.86),
            duration: 0.18,
            easing: Easing::QuadOut,
        },
    );
    // M26 damage-flash: attach an empty override block and the `damage_flash`
    // material id (if registered). Default zero overlay leaves the frame
    // byte-identical to the pre-M26 baseline — the tween in `systems.rs` is
    // what actually lights it up on a collision.
    world.insert(player, tungsten::core::UniformOverrideBlock::default());
    configure_platformer_camera(world, player);

    let ball_spawns: &[(f32, f32, f32)] = &[
        (10.0, 33.0, 280.0),
        (20.0, 29.0, -200.0),
        (29.0, 27.0, 360.0),
        (38.0, 33.0, -320.0),
        (51.0, 25.0, 220.0),
        (60.0, 31.0, -260.0),
        (77.0, 25.0, 300.0),
        (94.0, 15.0, -180.0),
        (119.0, 15.0, 240.0),
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
        world.insert(ball, tungsten::core::AnimationState::new(BALL_ANIMATION_ID));
        world.insert(ball, CurrentSprite(BALL_START_SPRITE_ID.into()));
    }
}

/// Sky, two cloud layers, ridges and woodland cover the actual camera bounds.
fn spawn_parallax_backdrop(world: &mut World) {
    for (id, y, scroll, z) in [
        ("ex10_sky", 0.0, 0.05, -300),
        ("ex10_clouds_far", 6.0 * TILE, 0.12, -280),
        ("ex10_clouds_near", 9.0 * TILE, 0.22, -250),
        ("ex10_distant_ridges", 10.0 * TILE, 0.35, -200),
        ("ex10_near_woodland", 18.0 * TILE, 0.6, -100),
    ] {
        let entity = world.spawn();
        world.insert(
            entity,
            Transform {
                position: Vec2::new(0.0, y),
                scale: if id.starts_with("ex10_clouds") {
                    Vec2::new(1.5, 0.8)
                } else {
                    Vec2::splat(2.0)
                },
                rotation: 0.0,
            },
        );
        if world.get_resource::<AssetRegistry>().is_none() {
            world.insert_resource(AssetRegistry::new());
        }
        let asset_id = world
            .get_resource_mut::<AssetRegistry>()
            .expect("AssetRegistry inserted above")
            .intern_sprite(id);
        let mut sprite = Sprite::new(asset_id);
        sprite.z_order = z;
        world.insert(entity, sprite);
        world.insert(entity, Visibility::default());
        world.insert(entity, ParallaxLayer::uniform(scroll));
    }
}

pub(crate) fn spawn_level_presentation(world: &mut World) {
    spawn_parallax_backdrop(world);
    for placement in PROPS {
        let entity = world.spawn();
        world.insert(entity, AnimatedProp(placement.depth));
        world.insert(
            entity,
            Transform::from_position(Vec2::from_array(placement.tile_position) * TILE),
        );
        world.insert(entity, CurrentSprite(placement.sprite.into()));
        if let Some(animation) = placement.animation {
            // All pieces of an animated set start together at phase zero.
            world.insert(entity, tungsten::core::AnimationState::new(animation));
        }
    }
    for placement in EMITTERS {
        let config = world
            .get_resource::<ParticleConfigRegistry>()
            .and_then(|registry| registry.id_for_name(placement.config))
            .expect("authored ambient particle config missing");
        let entity = world.spawn();
        world.insert(entity, AmbientEmitter);
        world.insert(
            entity,
            Transform::from_position(Vec2::from_array(placement.tile_position) * TILE),
        );
        world.insert(entity, ParticleEmitter::with_seed(config, placement.seed));
        world.insert(entity, ParticleEmitterState::default());
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
        // M30: idle shake stays off (`shake_amplitude` zero). Trauma from a hazard
        // hit rides this carrier and decays to nothing in under half a second.
        controller.shake_frequency_hz = 18.0;
        controller.shake_phase = 0.0;
        controller.shake_trauma = 0.0;
        controller.shake_decay = 2.2;
        controller.shake_max_offset = Vec2::new(0.22 * TILE, 0.12 * TILE);
    }
}

fn install_startup(app: &mut App) {
    app.on_startup(|world, _renderer| {
        // D-052: manifests loaded before startup; wire asset-dependent state.
        let registry = world.get_resource::<AssetRegistry>().unwrap();
        for id in [
            PLAYER_START_SPRITE_ID,
            BALL_START_SPRITE_ID,
            "ex10_sky",
            "ex10_distant_ridges",
            "ex10_near_woodland",
            "ex10_cursor",
        ] {
            assert!(registry.get_sprite(id).is_some(), "missing sprite '{id}'");
        }
        for placement in PROPS {
            assert!(
                registry.get_sprite(placement.sprite).is_some(),
                "missing prop '{}'",
                placement.sprite
            );
        }
        let animations = world.get_resource::<AnimationRegistry>().unwrap();
        for id in [
            PLAYER_ANIMATION_ID,
            BALL_ANIMATION_ID,
            "ex10_player_walk",
            "ex10_player_jump",
            "ex10_player_fall",
            "ex10_player_land",
            "ex10_player_double_jump",
            "ex10_fireball",
            "ex10_torch_flicker",
            "ex10_waterfall_flow",
            "ex10_vines_sway",
        ] {
            assert!(animations.get(id).is_some(), "missing animation '{id}'");
        }
        let tilemaps = world.get_resource::<TilemapRegistry>().unwrap();
        assert!(
            tilemaps.get("ex10_level").is_some(),
            "missing tilemap 'ex10_level'"
        );

        // Materials are registered only after manifests load, before startup.
        if let Some(material_id) = world
            .get_resource::<tungsten::core::MaterialRegistry>()
            .and_then(|r| r.get("damage_flash"))
        {
            for entity in world.query_entities::<Player>() {
                world.insert(entity, crate::state::PlayerMaterial { material_id });
            }
        }
        spawn_level_presentation(world);
        crate::gameplay::spawn_obstacles(world);

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
        let effect_sounds = {
            let reg = world
                .get_resource::<SoundRegistry>()
                .expect("SoundRegistry missing");
            let sound = |id: &str| {
                let handle = reg
                    .get_by_id(id)
                    .unwrap_or_else(|| panic!("{id} not found"));
                (handle, reg.get_volume(handle))
            };
            EffectSounds {
                cast: sound("ex10_fireball_cast_sfx"),
                blast: sound("ex10_fireball_blast_sfx"),
                extinguish: sound("ex10_extinguish_sfx"),
                extinguish_cooldown: 0.0,
            }
        };
        world.insert_resource(effect_sounds);
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
