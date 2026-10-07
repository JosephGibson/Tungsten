use super::*;

#[test]
fn extract_text_includes_debug_state_row() {
    let mut world = seed_world();
    world.insert_resource(TextDisplayState {
        fps: 60,
        contacts: 7,
        grounded: true,
        music_on: true,
        vol_pct: 50,
        zoom_pct: 125,
        ..TextDisplayState::default()
    });

    let sections = crate::extract::extract_text(&world);

    assert!(
        sections
            .iter()
            .any(|section| section.content.contains("FPS 60  Contacts 7  Grounded yes")),
        "debug state row should be visible in the example overlay"
    );
}

#[test]
fn update_text_display_refreshes_default_state_on_first_tick() {
    let mut world = seed_world();
    world.insert_resource(TextDisplayState::default());

    update_text_display(&mut world);

    let display = world.get_resource::<TextDisplayState>().unwrap();
    assert_eq!(display.fps, 60);
    assert!(
        display.timer < TEXT_UPDATE_INTERVAL,
        "first refresh should consume the primed display timer"
    );
}

#[test]
fn animation_transitions_hold_finish_face_and_interrupt_landing() {
    use crate::state::PlayerPresentation;
    use crate::systems::{animation_system, player_presentation_system};
    let mut harness = platformer_harness(&[
        ("player_input", player_input),
        ("physics_step", physics_step),
        ("ground_detection", ground_detection),
        ("player_presentation_system", player_presentation_system),
        ("animation_system", animation_system),
    ]);
    seed_level(harness.world_mut());
    load_presentation_assets(harness.world_mut());
    let player = spawn_test_player(harness.world_mut(), PLAYER_SPAWN);
    harness.step(5);
    let world = harness.world();
    assert_eq!(
        world.get::<AnimationState>(player).unwrap().animation_id,
        "ex10_player_idle"
    );
    assert_eq!(
        world
            .query::<(Entity, &crate::state::TransientEmitter)>()
            .count(),
        0,
        "spawn settles silently"
    );
    input_mut(&mut harness).key_down(KeyCode::KeyA);
    harness.step(1);
    let world = harness.world();
    assert_eq!(
        world.get::<AnimationState>(player).unwrap().animation_id,
        "ex10_player_walk"
    );
    assert!(world.get::<PlayerPresentation>(player).unwrap().facing_left);
    let input = input_mut(&mut harness);
    input.key_up(KeyCode::KeyA);
    input.key_down(KeyCode::Space);
    harness.step(1);
    let world = harness.world();
    assert_eq!(
        world.get::<AnimationState>(player).unwrap().animation_id,
        "ex10_player_jump"
    );
    assert!(
        !world.get::<Player>(player).unwrap().grounded,
        "previous floor contact must not ground the launch"
    );
    input_mut(&mut harness).key_up(KeyCode::Space);
    harness.step(13);
    assert!(
        harness
            .world()
            .get::<AnimationState>(player)
            .unwrap()
            .finished,
        "non-looping rise holds last pose"
    );
    let mut saw_fall = false;
    let mut saw_land = false;
    for _ in 0..65 {
        harness.step(1);
        let clip = harness
            .world()
            .get::<AnimationState>(player)
            .unwrap()
            .animation_id
            .as_str();
        saw_fall |= clip == "ex10_player_fall";
        if clip == "ex10_player_land" {
            saw_land = true;
            break;
        }
    }
    assert!(saw_fall && saw_land);
    input_mut(&mut harness).key_down(KeyCode::Space);
    harness.step(1);
    let world = harness.world();
    assert_eq!(
        world.get::<AnimationState>(player).unwrap().animation_id,
        "ex10_player_jump"
    );
    assert_eq!(
        world
            .get::<PlayerPresentation>(player)
            .unwrap()
            .landing_lock,
        0.0
    );
}

#[test]
fn jump_landing_bursts_fire_once_stay_at_event_and_cleanup() {
    use crate::state::TransientEmitter;
    use crate::systems::{player_presentation_system, transient_emitter_cleanup};
    let mut harness = platformer_harness(&[
        ("player_input", player_input),
        ("physics_step", physics_step),
        ("ground_detection", ground_detection),
        ("player_presentation_system", player_presentation_system),
        ("transient_emitter_cleanup", transient_emitter_cleanup),
    ]);
    seed_level(harness.world_mut());
    load_presentation_assets(harness.world_mut());
    let player = spawn_test_player(harness.world_mut(), PLAYER_SPAWN);
    harness.step(5);
    let takeoff =
        harness.world().get::<Position>(player).unwrap().0 + Vec2::new(0.0, PLAYER_HALF.y);
    input_mut(&mut harness).key_down(KeyCode::Space);
    harness.step(1);
    let world = harness.world();
    let emitter = world
        .query::<(Entity, &TransientEmitter)>()
        .next()
        .unwrap()
        .0;
    let origin = world.get::<Transform>(emitter).unwrap().position;
    assert_eq!(
        origin, takeoff,
        "jump puff belongs at the pre-physics feet position"
    );
    assert_eq!(world.query::<(Entity, &TransientEmitter)>().count(), 1);
    assert!(
        world.get::<TransientEmitter>(emitter).is_some(),
        "must survive first emission tick"
    );
    assert_eq!(
        world.query::<(Entity, &tungsten::core::Particle)>().count(),
        8
    );
    input_mut(&mut harness).key_up(KeyCode::Space);
    let mut landing_emitters = std::collections::HashSet::new();
    for _ in 0..100 {
        harness.step(1);
        let world = harness.world();
        for (entity, _) in world.query::<(Entity, &TransientEmitter)>() {
            if entity != emitter {
                landing_emitters.insert(entity);
            }
        }
        if let Some(t) = world.get::<Transform>(emitter) {
            assert_eq!(t.position, origin);
        }
    }
    let world = harness.world();
    assert!(world.get::<Player>(player).unwrap().grounded);
    assert_eq!(landing_emitters.len(), 1);
    assert_eq!(world.query::<(Entity, &TransientEmitter)>().count(), 0);
    assert_eq!(
        world.query::<(Entity, &tungsten::core::Particle)>().count(),
        0
    );
}

#[test]
fn particle_caps_and_ambient_placements_remain_bounded() {
    use tungsten::core::{
        Particle, ParticleBudget, ParticleConfigRegistry, ParticleEmitter, ParticleEmitterState,
    };
    let mut harness = platformer_harness(&[(
        "transient_emitter_cleanup",
        crate::systems::transient_emitter_cleanup,
    )]);
    let world = harness.world_mut();
    load_presentation_assets(world);
    crate::setup::spawn_level_presentation(world);
    let registry = world.get_resource::<ParticleConfigRegistry>().unwrap();
    let ambient_max: u32 = crate::level_layout::EMITTERS
        .iter()
        .map(|p| {
            registry
                .get(registry.id_for_name(p.config).unwrap())
                .unwrap()
                .max_alive
        })
        .sum();
    assert!(ambient_max <= 256);
    assert_eq!(
        world
            .query::<(Entity, &crate::state::AmbientEmitter)>()
            .count(),
        crate::level_layout::EMITTERS.len()
    );
    for _ in 0..30 {
        crate::systems::spawn_transient_effect(world, "ex10_landing_dust", Vec2::ZERO);
    }
    assert_eq!(
        world
            .query::<(Entity, &crate::state::TransientEmitter)>()
            .count(),
        crate::state::TRANSIENT_EMITTER_CAP
    );
    let config = world
        .get_resource::<ParticleConfigRegistry>()
        .unwrap()
        .id_for_name("ex10_black_hole")
        .unwrap();
    for i in 0..3 {
        let emitter = world.spawn();
        world.insert(emitter, Transform::default());
        world.insert(emitter, ParticleEmitter::with_seed(config, i));
        world.insert(emitter, ParticleEmitterState::default());
    }
    for _ in 0..240 {
        harness.step(1);
        let world = harness.world();
        assert!(
            world.query::<(Entity, &Particle)>().count()
                <= world.get_resource::<ParticleBudget>().unwrap().global_cap as usize
        );
    }
    assert_eq!(
        harness
            .world()
            .query::<(Entity, &crate::state::TransientEmitter)>()
            .count(),
        0
    );
}

#[test]
fn extraction_keeps_tile_stages_props_actors_foreground_and_cursor_in_order() {
    use crate::state::AnimatedProp;
    use tungsten::core::{AssetRegistry, InputState};
    let mut world = seed_world();
    let mut assets = AssetRegistry::new();
    for (id, name) in [
        "back",
        "decoration",
        "terrain",
        "foreground",
        "actor",
        "prop_back",
        "prop_world",
        "ex10_cursor",
    ]
    .iter()
    .enumerate()
    {
        mock_sprite(&mut assets, name, id as u32 + 1, false);
    }
    world.insert_resource(assets);
    let layers = [
        ("background", 0),
        ("decorations", 1),
        ("terrain", 2),
        ("foreground", 3),
        ("collision", 2),
    ]
    .into_iter()
    .map(|(name, tile)| TilemapLayer {
        name: name.into(),
        kind: if name == "collision" {
            LayerKind::Collision
        } else {
            LayerKind::Render
        },
        tiles: vec![tile],
    })
    .collect();
    world.get_resource_mut::<TilemapRegistry>().unwrap().insert(
        "test".into(),
        TilemapData {
            width: 1,
            height: 1,
            tile_width: 64,
            tile_height: 64,
            tileset: ["back", "decoration", "terrain", "foreground"]
                .map(str::to_owned)
                .into(),
            layers,
        },
    );
    let map = world.spawn();
    world.insert(map, TilemapInstance::new("test", Vec2::ZERO));
    let player = spawn_test_player(&mut world, Vec2::new(32.0, 32.0));
    world.insert(player, CurrentSprite("actor".into()));
    for (name, depth) in [
        ("prop_back", crate::level_layout::PropDepth::Back),
        ("prop_world", crate::level_layout::PropDepth::World),
    ] {
        let entity = world.spawn();
        world.insert(entity, AnimatedProp(depth));
        world.insert(entity, Transform::default());
        world.insert(entity, CurrentSprite(name.into()));
    }
    world
        .get_resource_mut::<InputState>()
        .unwrap()
        .update_cursor_position(10.0, 10.0);
    let extracted = crate::extract::extract_sprites(&world);
    let handles: Vec<_> = extracted.iter().map(|b| b.texture.0).collect();
    assert_eq!(handles, vec![1, 2, 6, 3, 7, 5, 4, 8]);
    // Moving the view culls all offscreen tile layers and props.
    world.get_resource_mut::<CameraState>().unwrap().position = Vec2::splat(1000.0);
    assert!(
        crate::extract::extract_tile_layers(
            &world,
            &[
                "background",
                "decorations",
                "terrain",
                "foreground",
                "collision"
            ]
        )
        .is_empty()
    );
}

#[test]
fn all_player_orb_frames_keep_lighting_material_facing_and_bottom_anchor() {
    use crate::state::{LightingFixture, LightingFixtureMode, PlayerMaterial, PlayerPresentation};
    use tungsten::core::{AnimationRegistry, AssetRegistry, MaterialAssetId, UniformOverrideBlock};
    let mut world = seed_world();
    load_presentation_assets(&mut world);
    let mut assets = world.remove_resource::<AssetRegistry>().unwrap();
    let manifest =
        tungsten::core::assets::manifest::ResolvedManifest::load(asset_path("manifest.json"))
            .unwrap();
    let frames: Vec<_> = world
        .get_resource::<AnimationRegistry>()
        .unwrap()
        .iter()
        .filter(|(name, _)| name.starts_with("ex10_player") || *name == "ex10_ball_spin")
        .flat_map(|(_, clip)| clip.frames.iter().map(|f| f.sprite))
        .map(|id| assets.sprite_name(id).unwrap().to_owned())
        .collect::<Vec<_>>();
    for (i, name) in frames.iter().enumerate() {
        let sprite = manifest.sprites.get(name).unwrap();
        assert!(
            sprite.normal_path.is_some() && sprite.emissive_path.is_some(),
            "{name}"
        );
        mock_sprite(&mut assets, name, i as u32, true);
    }
    world.insert_resource(assets);
    let player = spawn_test_player(&mut world, Vec2::new(100.0, 100.0));
    world.insert(
        player,
        PlayerMaterial {
            material_id: MaterialAssetId(0),
        },
    );
    world.insert(player, UniformOverrideBlock::default());
    world.get_mut::<Transform>(player).unwrap().scale = Vec2::new(1.16, 0.86);
    for name in frames.iter().filter(|name| name.starts_with("ex10_player")) {
        world.insert(player, CurrentSprite(name.clone()));
        for mode in [LightingFixtureMode::On, LightingFixtureMode::Off] {
            world.insert_resource(LightingFixture { mode });
            for left in [false, true] {
                world
                    .get_mut::<PlayerPresentation>(player)
                    .unwrap()
                    .facing_left = left;
                let batches = crate::extract::extract_sprites(&world);
                assert_eq!(batches.len(), 1);
                let batch = &batches[0];
                let sprite = &batch.instances[0];
                assert_eq!(batch.lit, mode == LightingFixtureMode::On);
                assert_eq!(
                    batch.material_id.is_some(),
                    mode == LightingFixtureMode::Off
                );
                assert_eq!(sprite.uv_size[0] < 0.0, left);
                assert!((sprite.position[1] + 61.0 * 0.86 - 128.0).abs() < 0.001);
            }
        }
    }
    world.despawn(player);
    let ball = world.spawn();
    world.insert(ball, Ball);
    world.insert(ball, Position(Vec2::new(200.0, 200.0)));
    world.insert_resource(LightingFixture {
        mode: LightingFixtureMode::On,
    });
    for name in frames.iter().filter(|name| name.starts_with("ex10_ball")) {
        world.insert(ball, CurrentSprite(name.clone()));
        let batches = crate::extract::extract_sprites(&world);
        assert!(batches[0].lit);
        assert_eq!(batches[0].instances[0].position, [184.0, 184.0]);
    }
    // A ball outside the view is not extracted.
    world.get_resource_mut::<CameraState>().unwrap().position = Vec2::splat(1000.0);
    assert!(crate::extract::extract_sprites(&world).is_empty());
}

#[test]
fn landing_clip_completes_without_resetting_idle_and_props_stay_synchronized() {
    use crate::systems::{animation_system, player_presentation_system};
    let mut harness = platformer_harness(&[
        ("player_presentation_system", player_presentation_system),
        ("animation_system", animation_system),
    ]);
    let world = harness.world_mut();
    load_presentation_assets(world);
    crate::setup::spawn_level_presentation(world);
    let player = spawn_test_player(world, PLAYER_SPAWN);
    world.get_mut::<Player>(player).unwrap().grounded = true;
    world
        .get_mut::<crate::state::PlayerPresentation>(player)
        .unwrap()
        .pending_effect = Some(crate::state::PlayerEffect::Land);
    harness.step(1);
    assert_eq!(
        harness
            .world()
            .get::<AnimationState>(player)
            .unwrap()
            .animation_id,
        "ex10_player_land"
    );
    harness.step(20);
    let world = harness.world_mut();
    assert_eq!(
        world.get::<AnimationState>(player).unwrap().animation_id,
        "ex10_player_idle"
    );
    let before = world.get::<AnimationState>(player).unwrap().accumulated_ms;
    // Selection alone, without a frame's animation advance, keeps the clip.
    player_presentation_system(world);
    assert_eq!(
        world.get::<AnimationState>(player).unwrap().accumulated_ms,
        before
    );
    let waterfalls: Vec<_> = world
        .query::<(Entity, &AnimationState)>()
        .filter(|(_, a)| a.animation_id == "ex10_waterfall_flow")
        .collect();
    assert_eq!(waterfalls.len(), 48);
    let first = waterfalls[0].1;
    assert!(waterfalls.iter().all(
        |(_, a)| a.frame_index == first.frame_index && a.accumulated_ms == first.accumulated_ms
    ));
}

#[test]
fn cloud_parallax_and_vortex_instances_animate_without_world_mutation() {
    use tungsten::core::AssetRegistry;
    let mut world = seed_world();
    load_presentation_assets(&mut world);
    crate::setup::spawn_level_presentation(&mut world);
    let mut assets = AssetRegistry::new();
    for (i, name) in [
        "ex10_clouds_far",
        "ex10_vortex",
        "ex10_vortex_core",
        "ex10_spark",
    ]
    .iter()
    .enumerate()
    {
        mock_sprite(&mut assets, name, i as u32, false);
    }
    world.insert_resource(assets);
    let hole = world.spawn();
    world.insert(hole, BlackHole { remaining: 2.0 });
    world.insert(hole, Position(Vec2::new(300.0, 200.0)));
    world.insert_resource(crate::gameplay::SceneTime(0.0));
    let first = crate::extract::extract_sprites(&world);
    world
        .get_resource_mut::<crate::gameplay::SceneTime>()
        .unwrap()
        .0 = 0.25;
    let second = crate::extract::extract_sprites(&world);
    assert_ne!(
        first[0].instances[0].position,
        second[0].instances[0].position
    );
    assert!(
        second
            .iter()
            .flat_map(|b| &b.instances)
            .any(|i| i.rotation.abs() > 0.5)
    );
    assert_eq!(
        world.get::<Position>(hole).unwrap().0,
        Vec2::new(300.0, 200.0)
    );
    assert_eq!(world.query::<(Entity, &BlackHole)>().count(), 1);
}

#[test]
fn player_lantern_toggle_controls_halo_and_native_light_and_tracks_facing() {
    use crate::gameplay::*;
    use tungsten::core::{AssetRegistry, Light};
    let mut harness = platformer_harness(&[("lantern_input", lantern_input)]);
    let world = harness.world_mut();
    let player = spawn_test_player(world, PLAYER_SPAWN);
    spawn_obstacles(world);
    scene_effects(world);
    let center = glow_center(world, player, Vec2::ZERO).unwrap();
    assert!(center.x > PLAYER_SPAWN.x && center.y < PLAYER_SPAWN.y + PLAYER_HALF.y);
    world
        .get_mut::<crate::state::PlayerPresentation>(player)
        .unwrap()
        .facing_left = true;
    let left = glow_center(world, player, Vec2::ZERO).unwrap();
    assert!((left.x + center.x - 2.0 * PLAYER_SPAWN.x).abs() < 0.001);
    world.get_resource_mut::<CameraState>().unwrap().position =
        PLAYER_SPAWN - Vec2::new(200.0, 150.0);
    let mut assets = AssetRegistry::new();
    mock_sprite(&mut assets, "ex10_halo", 901, false);
    mock_sprite(&mut assets, "ex10_flame_glow", 902, false);
    world.insert_resource(assets);
    let on = crate::extract::extract_sprites(world)
        .iter()
        .map(|b| b.instances.len())
        .sum::<usize>();
    input_mut(&mut harness).key_down(KeyCode::KeyL);
    harness.step(1);
    let world = harness.world_mut();
    scene_effects(world);
    assert!(glow_center(world, player, Vec2::ZERO).is_none());
    assert_eq!(
        world
            .query::<(Entity, &Light)>()
            .filter(|(_, l)| l.intensity == 0.0)
            .count(),
        1
    );
    let off = crate::extract::extract_sprites(world)
        .iter()
        .map(|b| b.instances.len())
        .sum::<usize>();
    assert_eq!(on - off, 2);
    harness.step(1);
    assert!(
        !harness
            .world()
            .get::<PlayerLantern>(player)
            .unwrap()
            .enabled,
        "held key must not flicker"
    );
    let input = input_mut(&mut harness);
    input.key_up(KeyCode::KeyL);
    input.key_down(KeyCode::KeyL);
    harness.step(1);
    let world = harness.world_mut();
    scene_effects(world);
    assert!(world.get::<PlayerLantern>(player).unwrap().enabled);
    assert!(
        world
            .query::<(Entity, &Light)>()
            .all(|(_, l)| l.intensity > 0.0)
    );
}

#[test]
fn hearts_reflect_every_hp_value_immediately_at_constant_screen_size() {
    use tungsten::core::AssetRegistry;
    let mut world = seed_world();
    let player = spawn_test_player(&mut world, PLAYER_SPAWN);
    world.insert(player, crate::gameplay::Health::default());
    let mut assets = AssetRegistry::new();
    mock_sprite(&mut assets, "ex10_heart_full", 801, false);
    mock_sprite(&mut assets, "ex10_heart_empty", 802, false);
    world.insert_resource(assets);
    for zoom in [0.35, 1.0, 3.0] {
        world.get_resource_mut::<CameraState>().unwrap().zoom = zoom;
        world.get_resource_mut::<CameraState>().unwrap().position = Vec2::new(800.0, 1500.0);
        for hp in 0..=3 {
            world
                .get_mut::<crate::gameplay::Health>(player)
                .unwrap()
                .hearts = hp;
            let batches = crate::extract::extract_sprites(&world);
            assert_eq!(batches.iter().map(|b| b.instances.len()).sum::<usize>(), 3);
            assert_eq!(
                batches
                    .iter()
                    .filter(|b| b.texture.0 == 801)
                    .map(|b| b.instances.len())
                    .sum::<usize>(),
                hp as usize
            );
            for (index, heart) in batches.iter().flat_map(|b| &b.instances).enumerate() {
                assert!((heart.size[0] * zoom - 40.0).abs() < 0.001);
                let screen = (Vec2::from_array(heart.position) - Vec2::new(800.0, 1500.0)) * zoom;
                assert!((screen - Vec2::new(16.0 + index as f32 * 40.0, 86.0)).length() < 0.001);
            }
        }
    }
}

#[test]
fn midnight_uses_restrained_stock_post_passes_and_round_moon_at_all_aspects() {
    use tungsten::core::{
        AssetRegistry,
        post::{PostPass, PostStack},
    };
    let mut app = App::new(Config::default()).unwrap();
    crate::setup::configure_app(&mut app);
    let stack = app.world_mut().get_resource::<PostStack>().unwrap();
    assert!(
        matches!(stack.0.as_slice(),[PostPass::Bloom(b),PostPass::Vignette(v)] if b.intensity<=0.3 && v.strength<=0.2)
    );
    let mut world = seed_world();
    load_presentation_assets(&mut world);
    crate::setup::spawn_level_presentation(&mut world);
    // The backdrop interned its names in the world's registry; mock them there.
    let mut assets = world.remove_resource::<AssetRegistry>().unwrap();
    mock_sprite(&mut assets, "ex10_sky", 700, false);
    mock_sprite(&mut assets, "ex10_moon", 701, false);
    world.insert_resource(assets);
    for (width, height) in [(1920, 1080), (800, 800), (3840, 720)] {
        world.insert_resource(WindowSize { width, height });
        let batches = crate::extract::extract_sprites(&world);
        let moon = &batches
            .iter()
            .find(|b| b.texture.0 == 701)
            .unwrap()
            .instances[0];
        assert_eq!(moon.size[0], moon.size[1]);
        assert_eq!(moon.rotation, 0.0);
    }
}

#[test]
fn glows_draw_through_soft_materials_without_absorbing_other_sprites() {
    use crate::gameplay::Glow;
    use tungsten::core::{AssetRegistry, MaterialRegistry, MaterialUniformDefaults};
    let mut world = seed_world();
    let mut materials = MaterialRegistry::new();
    let halo = materials.allocate(
        "ex10_soft_halo",
        "halo".into(),
        "ex10_soft_glow".into(),
        MaterialUniformDefaults::default(),
    );
    world.insert_resource(materials);
    let mut assets = AssetRegistry::new();
    // Both sprites share one atlas page, as packed sprites do.
    mock_sprite(&mut assets, "ex10_halo", 7, false);
    mock_sprite(&mut assets, "ex10_lantern", 7, false);
    world.insert_resource(assets);
    for x in [100.0, 180.0] {
        let lamp = world.spawn();
        world.insert(lamp, Transform::from_position(Vec2::new(x, 100.0)));
        world.insert(
            lamp,
            Glow {
                offset: Vec2::ZERO,
                radius: 40.0,
                color: [255, 190, 100, 255],
            },
        );
    }
    let batches = crate::extract::extract_sprites(&world);
    let glow_batches: Vec<_> = batches
        .iter()
        .filter(|b| b.material_id == Some(halo))
        .collect();
    assert_eq!(glow_batches.len(), 1);
    assert_eq!(glow_batches[0].instances.len(), 2);
    assert!(
        batches
            .iter()
            .filter(|b| b.material_id.is_none() && b.texture.0 == 7)
            .all(|b| b.instances.iter().all(|i| i.size != [80.0, 80.0]))
    );
}
