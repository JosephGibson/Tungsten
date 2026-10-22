//! What draws: the courier's clips and bursts, the ambient emitters and
//! the particle caps, the drawing order, the lit frames' feet, the props'
//! animations in step, the lantern's glow and light, midnight's post stack
//! and moon, and the glows' material batches.

use std::path::PathBuf;

use example_01_platformer::features::level::components::{AmbientEmitter, Glow, GlowParts};
use example_01_platformer::features::level::draw;
use example_01_platformer::features::level::systems::HALO_MATERIAL;
use example_01_platformer::features::player::components::{LanternAnchors, Moves};
use example_01_platformer::features::player::systems::{JUMP_PUFF, LANDING_DUST};
use example_01_platformer::features::player::{BODY_OFFSET, PLAYER_HALF};
use example_01_platformer::game::GAMEPLAY;
use example_01_platformer::shared::{BURST_CAP, Burst, spawn_burst};
use example_01_platformer::shell::options::GameOptions;
use glam::Vec2;
use tungsten::core::assets::{LayerKind, ResolvedManifest, TilemapData, TilemapLayer};
use tungsten::core::post::{PostPass, PostStack};
use tungsten::core::{
    AnimationRegistry, AnimationState, AssetRegistry, CameraState, DataRegistry, Entity,
    InputState, Light, MaterialRegistry, Particle, ParticleBudget, ParticleConfigRegistry,
    ParticleEmitter, ParticleEmitterState, Position, Sprite, TilemapInstance, TilemapRegistry,
    Transform, Visibility,
};
use tungsten::kit::{Facing, PlatformerController, damage};
use tungsten::testing::UiHarness;
use tungsten::{SceneEntity, WindowSize, extract_sprites_default, extract_tilemap_layers};

use example_01_platformer::features::level::components::SceneTime;
use example_01_platformer::features::toys::{self, components::BlackHole};

use super::{centre, mock_sprite, press, run, settle, teleport};

fn clip(harness: &UiHarness, player: Entity) -> String {
    let state = harness.world().get::<AnimationState>(player).unwrap();
    state.animation_id.clone()
}

fn grounded(harness: &UiHarness, player: Entity) -> bool {
    let world = harness.world();
    world
        .get::<PlatformerController>(player)
        .is_some_and(PlatformerController::grounded)
}

fn bursts(harness: &UiHarness) -> Vec<Entity> {
    let world = harness.world();
    world
        .query::<(Entity, &Burst)>()
        .map(|(entity, _)| entity)
        .collect()
}

/// The particles bursts own.
fn burst_particles(harness: &UiHarness) -> usize {
    let world = harness.world();
    world
        .query::<&Particle>()
        .filter(|particle| {
            particle
                .emitter
                .is_some_and(|emitter| world.has::<Burst>(emitter))
        })
        .count()
}

#[test]
fn animation_transitions_hold_finish_face_and_interrupt_landing() {
    let (mut harness, player) = run();
    harness.step(5);
    assert_eq!(clip(&harness, player), "ex10_player_idle");
    assert!(bursts(&harness).is_empty(), "the spawn settles silently");
    harness.harness_mut().press_action("move_left");
    harness.step(1);
    assert_eq!(clip(&harness, player), "ex10_player_walk_left");
    let facing = harness.world().get::<Facing>(player).unwrap().direction();
    assert!(facing.x < 0.0);
    harness.harness_mut().release_action("move_left");
    harness.harness_mut().press_action("jump");
    harness.step(1);
    assert_eq!(clip(&harness, player), "ex10_player_jump_left");
    assert!(
        !grounded(&harness, player),
        "the floor under the launch keeps nothing"
    );
    // Held through the rise, so the cut never shortens it.
    harness.step(13);
    let state = harness.world().get::<AnimationState>(player).unwrap();
    assert!(state.finished, "the rise holds its last pose");
    harness.harness_mut().release_action("jump");
    let (mut saw_fall, mut saw_land) = (false, false);
    for _ in 0..65 {
        harness.step(1);
        let clip = clip(&harness, player);
        saw_fall |= clip == "ex10_player_fall_left";
        if clip == "ex10_player_land_left" {
            saw_land = true;
            break;
        }
    }
    assert!(saw_fall && saw_land);
    harness.harness_mut().press_action("jump");
    harness.step(1);
    assert_eq!(clip(&harness, player), "ex10_player_jump_left");
    assert_eq!(harness.world().get::<Moves>(player).unwrap().landing, 0.0);
}

#[test]
fn jump_landing_bursts_fire_once_stay_at_event_and_cleanup() {
    let (mut harness, player) = run();
    harness.step(5);
    let takeoff = centre(&harness, player) + Vec2::new(0.0, PLAYER_HALF.y);
    harness.harness_mut().press_action("jump");
    harness.step(1);
    let world = harness.world();
    let puff = bursts(&harness);
    assert_eq!(puff.len(), 1);
    let puff = puff[0];
    let config = world
        .resource::<ParticleConfigRegistry>()
        .id_for_name(JUMP_PUFF);
    assert_eq!(
        Some(world.get::<ParticleEmitter>(puff).unwrap().config),
        config
    );
    let origin = world.get::<Transform>(puff).unwrap().position;
    assert_eq!(origin, takeoff, "the puff belongs at the feet it left from");
    assert_eq!(
        burst_particles(&harness),
        8,
        "it survives its first emission"
    );
    harness.harness_mut().release_action("jump");
    let mut landings = std::collections::HashSet::new();
    for _ in 0..100 {
        harness.step(1);
        let world = harness.world();
        for burst in bursts(&harness) {
            if burst != puff {
                landings.insert(burst);
            }
        }
        if let Some(at) = world.get::<Transform>(puff) {
            assert_eq!(at.position, origin);
        }
    }
    assert!(grounded(&harness, player));
    assert_eq!(landings.len(), 1, "one landing's dust");
    let dust = harness
        .world()
        .resource::<ParticleConfigRegistry>()
        .id_for_name(LANDING_DUST);
    assert!(dust.is_some());
    assert!(
        bursts(&harness).is_empty(),
        "both bursts went with their particles"
    );
    assert_eq!(burst_particles(&harness), 0);
}

#[test]
fn particle_caps_and_ambient_placements_remain_bounded() {
    let (mut harness, _) = run();
    let world = harness.world();
    let registry = world.resource::<ParticleConfigRegistry>();
    // The map's emitters: each object at its place, with its config and seed.
    let map = world
        .resource::<TilemapRegistry>()
        .get("ex10_level")
        .unwrap();
    let objects: Vec<_> = map
        .object_layers
        .iter()
        .filter(|layer| layer.name == "emitters")
        .flat_map(|layer| &layer.objects)
        .collect();
    let spawned: Vec<(Vec2, ParticleEmitter)> = world
        .query::<(&AmbientEmitter, &Transform, &ParticleEmitter)>()
        .map(|(_, at, emitter)| (at.position, *emitter))
        .collect();
    assert_eq!(spawned.len(), objects.len());
    assert_eq!(spawned.len(), 7);
    let ambient: u32 = spawned
        .iter()
        .map(|(_, emitter)| registry.get(emitter.config).unwrap().max_alive)
        .sum();
    assert!(ambient <= 256, "{ambient}");
    for object in objects {
        let property = |name: &str| object.properties.get(name).cloned().unwrap();
        let config = registry.id_for_name(property("particle_emitter.config").as_str().unwrap());
        let seed = property("particle_emitter.seed").as_u64();
        assert!(
            spawned.iter().any(|(at, emitter)| {
                (*at - object.position).abs().max_element() <= 1e-3
                    && Some(emitter.config) == config
                    && emitter.seed_override == seed
            }),
            "emitter at {}",
            object.position
        );
    }
    let world = harness.world_mut();
    for _ in 0..30 {
        spawn_burst(world, LANDING_DUST, Vec2::new(500.0, 500.0));
    }
    assert_eq!(world.query::<&Burst>().count(), BURST_CAP);
    let hole = world
        .resource::<ParticleConfigRegistry>()
        .id_for_name("ex10_black_hole")
        .unwrap();
    for seed in 0..3 {
        world.spawn_with((
            Transform::from_position(Vec2::new(600.0, 2000.0)),
            ParticleEmitter::with_seed(hole, seed),
            ParticleEmitterState::default(),
            SceneEntity { state_id: GAMEPLAY },
        ));
    }
    let cap = world.resource::<ParticleBudget>().global_cap as usize;
    assert_eq!(cap, 2048);
    for _ in 0..240 {
        harness.step(1);
        assert!(harness.world().query::<&Particle>().count() <= cap);
    }
    assert!(bursts(&harness).is_empty());
}

#[test]
fn extraction_keeps_tile_stages_props_actors_foreground_and_cursor_in_order() {
    let (mut harness, player) = run();
    let world = harness.world_mut();
    let names = [
        "ex10_sky",
        "t_back",
        "t_decoration",
        "t_terrain",
        "t_foreground",
        "t_prop_back",
        "t_prop_world",
        "t_actor",
        "ex10_cursor",
    ];
    let assets = world.resource_mut::<AssetRegistry>();
    for (i, name) in names.iter().enumerate() {
        mock_sprite(assets, name, 501 + i as u32, false);
    }
    // A one-tile map of four drawn layers and a collision layer, far from
    // the level, where the view now looks.
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
    let mut map = TilemapData::new(64, 64, 1, 1);
    map.tileset = ["t_back", "t_decoration", "t_terrain", "t_foreground"]
        .map(str::to_owned)
        .into();
    map.layers = layers;
    world
        .resource_mut::<TilemapRegistry>()
        .insert("test".into(), map);
    let far = Vec2::splat(20_000.0);
    world.spawn_with((
        TilemapInstance::new("test", far),
        SceneEntity { state_id: GAMEPLAY },
    ));
    for (name, z_order) in [("t_prop_world", -10), ("t_actor", 10), ("t_prop_back", -20)] {
        let asset_id = world.resource::<AssetRegistry>().sprite_id(name).unwrap();
        let sprite = Sprite {
            z_order,
            ..Sprite::new(asset_id)
        };
        world.spawn_with((Transform::from_position(far), sprite, Visibility::default()));
    }
    let _ = player;
    world.insert_resource(WindowSize {
        width: 480,
        height: 288,
    });
    let camera = world.resource_mut::<CameraState>();
    camera.position = far - Vec2::splat(10.0);
    camera.zoom = 1.0;
    world
        .resource_mut::<InputState>()
        .update_cursor_position(10.0, 10.0);
    let world = harness.world();
    let mut batches = draw::base(world);
    batches.extend(draw::cursor(world));
    let handles: Vec<u32> = batches
        .iter()
        .map(|batch| batch.texture.0)
        .filter(|handle| (501..=509).contains(handle))
        .collect();
    // The sky, the tiles by layer, the sprites by depth, the cursor last.
    assert_eq!(handles, [501, 502, 503, 504, 505, 506, 507, 508, 509]);
    // A view elsewhere culls every tile layer of that map.
    harness.world_mut().resource_mut::<CameraState>().position = Vec2::splat(30_000.0);
    let layers = [
        "background",
        "decorations",
        "terrain",
        "foreground",
        "collision",
    ];
    assert!(extract_tilemap_layers(harness.world(), &layers).is_empty());
}

#[test]
fn all_player_orb_frames_keep_lighting_material_facing_and_bottom_anchor() {
    let (mut harness, player) = run();
    harness.step(5);
    let manifest = ResolvedManifest::load(PathBuf::from("assets/manifest.json")).unwrap();
    let world = harness.world_mut();
    let frames: Vec<String> = {
        let assets = world.resource::<AssetRegistry>();
        let mut frames: Vec<String> = world
            .resource::<AnimationRegistry>()
            .iter()
            .filter(|(name, _)| name.starts_with("ex10_player"))
            .flat_map(|(_, clip)| clip.frames.iter().map(|frame| frame.sprite))
            .map(|id| assets.sprite_name(id).unwrap().to_owned())
            .collect();
        frames.sort();
        frames.dedup();
        frames
    };
    assert!(
        frames.iter().any(|name| name.ends_with("_left")),
        "mirrored frames"
    );
    for (i, name) in frames.iter().enumerate() {
        let sprite = manifest.sprites.get(name).unwrap();
        assert!(
            sprite.normal_path.is_some() && sprite.emissive_path.is_some(),
            "{name} draws lit"
        );
        mock_sprite(
            world.resource_mut::<AssetRegistry>(),
            name,
            600 + i as u32,
            true,
        );
    }
    let feet = world.get::<Position>(player).unwrap().0 + BODY_OFFSET + Vec2::Y * PLAYER_HALF.y;
    for (i, name) in frames.iter().enumerate() {
        let id = world.resource::<AssetRegistry>().sprite_id(name).unwrap();
        world.get_mut::<Sprite>(player).unwrap().asset_id = id;
        let batches = extract_sprites_default(world);
        let batch = batches
            .iter()
            .find(|batch| batch.texture.0 == 600 + i as u32)
            .unwrap_or_else(|| panic!("{name} drew"));
        assert!(batch.lit, "{name} draws lit");
        assert!(
            batch.material_id.is_none(),
            "lit, so no flash material (Q6)"
        );
        let sprite = &batch.instances[0];
        assert!(
            sprite.uv_size[0] > 0.0,
            "mirrored art, never a flipped quad"
        );
        // The art's feet, its row 61, stand on the body's bottom.
        assert!((sprite.position[1] + 61.0 - feet.y).abs() < 1e-3, "{name}");
    }
}

#[test]
fn landing_clip_completes_without_resetting_idle_and_props_stay_synchronized() {
    let (mut harness, player) = run();
    harness.step(10);
    let stood = centre(&harness, player);
    teleport(&mut harness, player, stood - Vec2::new(0.0, 100.0));
    harness.step(1);
    let mut frames = 0;
    while !grounded(&harness, player) {
        harness.step(1);
        frames += 1;
        assert!(frames < 60);
    }
    harness.step(1);
    assert_eq!(clip(&harness, player), "ex10_player_land");
    harness.step(20);
    assert_eq!(clip(&harness, player), "ex10_player_idle");
    let before = harness
        .world()
        .get::<AnimationState>(player)
        .unwrap()
        .accumulated_ms;
    // A frame of no time picks again and keeps the clip where it was.
    harness.harness_mut().set_dt(0.0);
    harness.step(1);
    let after = harness
        .world()
        .get::<AnimationState>(player)
        .unwrap()
        .accumulated_ms;
    assert_eq!(before, after);
    let world = harness.world();
    // The ravine's falls: its lip, its spread, its curtain's two columns
    // and its foot, every piece a clip of the same frames.
    let waterfalls: Vec<&AnimationState> = world
        .query::<&AnimationState>()
        .filter(|state| state.animation_id.starts_with("ex10_falls_"))
        .collect();
    assert_eq!(waterfalls.len(), 67);
    let first = waterfalls[0];
    assert!(waterfalls.iter().all(|state| {
        state.frame_index == first.frame_index && state.accumulated_ms == first.accumulated_ms
    }));
}

#[test]
fn player_lantern_toggle_controls_halo_and_native_light_and_tracks_facing() {
    let (mut harness, player) = run();
    harness.step(5);
    let world = harness.world();
    let anchors = world
        .resource::<DataRegistry>()
        .get::<LanternAnchors>("player")
        .unwrap()
        .clone();
    // A left-facing frame's flame mirrors its right-facing twin's about the
    // body's centre line, x 32 of the frame.
    for (name, [x, y]) in &anchors.0 {
        if let Some([left, left_y]) = anchors.0.get(&format!("{name}_left")) {
            assert!(
                (x + left - 2.0 * BODY_OFFSET.x).abs() < 1e-3 && y == left_y,
                "{name}"
            );
        }
    }
    let shown = |harness: &UiHarness| {
        let world = harness.world();
        let sprite = world.get::<Sprite>(player).unwrap().asset_id;
        let name = world
            .resource::<AssetRegistry>()
            .sprite_name(sprite)
            .unwrap()
            .to_owned();
        (name, *world.get::<Glow>(player).unwrap())
    };
    let (name, glow) = shown(&harness);
    assert_eq!(
        Some(&glow.offset),
        anchors.0.get(&name),
        "on {name}'s flame"
    );
    let at = harness.world().get::<Transform>(player).unwrap().position;
    let flame = glow.centre(at);
    assert!(
        flame.x > centre(&harness, player).x,
        "the lantern hangs ahead"
    );
    harness.harness_mut().press_action("move_left");
    harness.step(1);
    harness.harness_mut().release_action("move_left");
    harness.step(1);
    let (name, glow) = shown(&harness);
    assert!(name.ends_with("_left"));
    assert_eq!(
        Some(&glow.offset),
        anchors.0.get(&name),
        "on {name}'s flame"
    );
    // Lit: the halo and the core draw and the light shines.
    let world = harness.world_mut();
    let assets = world.resource_mut::<AssetRegistry>();
    mock_sprite(assets, "ex10_halo", 901, false);
    mock_sprite(assets, "ex10_flame_glow", 902, false);
    let parts = *world.get::<GlowParts>(player).unwrap();
    let count = |harness: &UiHarness| -> usize {
        extract_sprites_default(harness.world())
            .iter()
            .filter(|batch| [901, 902].contains(&batch.texture.0))
            .map(|batch| batch.instances.len())
            .sum()
    };
    harness.step(1);
    let on = count(&harness);
    assert!(harness.world().get::<Light>(parts.light).unwrap().intensity > 0.0);
    harness.harness_mut().press_action("lantern");
    harness.step(1);
    let world = harness.world();
    assert!(world.get::<Glow>(player).unwrap().hidden);
    assert_eq!(world.get::<Light>(parts.light).unwrap().intensity, 0.0);
    let dark = world
        .query::<&Light>()
        .filter(|light| light.intensity == 0.0)
        .count();
    assert_eq!(dark, 1, "the lantern's light alone");
    assert_eq!(on - count(&harness), 2, "its halo and core go");
    harness.step(1);
    assert!(
        harness.world().get::<Glow>(player).unwrap().hidden,
        "a held key never flickers"
    );
    harness.harness_mut().release_action("lantern");
    harness.harness_mut().press_action("lantern");
    harness.step(1);
    let world = harness.world();
    assert!(!world.get::<Glow>(player).unwrap().hidden);
    assert!(world.query::<&Light>().all(|light| light.intensity > 0.0));
}

#[test]
fn bloom_and_vignette_survive_game_over_and_retry() {
    let (mut harness, player) = run();
    let stack = |harness: &UiHarness| harness.world().resource::<PostStack>().0.clone();
    let night = stack(&harness);
    let pair = matches!(
        night.as_slice(),
        [PostPass::Bloom(_), PostPass::Vignette(_)]
    );
    assert!(pair, "{night:?}");
    damage(harness.world_mut(), player, 1000.0);
    harness.step(2);
    settle(&mut harness);
    let dead = stack(&harness);
    assert_eq!(dead[..2], night[..], "under game over's passes: {dead:?}");
    press(&mut harness, "Retry");
    assert_eq!(stack(&harness), night, "after Retry");
}

#[test]
fn the_bloom_option_keeps_the_vignette() {
    let (mut harness, _) = run();
    let set = |harness: &mut UiHarness, on: bool| {
        harness.world_mut().resource_mut::<GameOptions>().bloom = on;
        harness.step(1);
        harness.world().resource::<PostStack>().0.clone()
    };
    let off = set(&mut harness, false);
    assert!(matches!(off.as_slice(), [PostPass::Vignette(_)]), "{off:?}");
    let on = set(&mut harness, true);
    let both = matches!(on.as_slice(), [PostPass::Bloom(_), PostPass::Vignette(_)]);
    assert!(both, "{on:?}");
}

#[test]
fn midnight_uses_restrained_stock_post_passes_and_round_moon_at_all_aspects() {
    let (mut harness, _) = run();
    let world = harness.world_mut();
    let stack = world.resource::<PostStack>();
    assert!(
        matches!(stack.0.as_slice(), [PostPass::Bloom(b), PostPass::Vignette(v)] if b.intensity <= 0.3 && v.strength <= 0.2),
        "{stack:?}"
    );
    let assets = world.resource_mut::<AssetRegistry>();
    mock_sprite(assets, "ex10_sky", 700, false);
    mock_sprite(assets, draw::MOON, 701, false);
    for (width, height) in [(1920, 1080), (800, 800), (3840, 720)] {
        world.insert_resource(WindowSize { width, height });
        let batches = draw::backdrop(world);
        let moon = &batches
            .iter()
            .find(|b| b.texture.0 == 701)
            .unwrap()
            .instances[0];
        assert_eq!(moon.size[0], moon.size[1], "round at {width}x{height}");
        assert_eq!(moon.rotation, 0.0);
    }
}

#[test]
fn glows_draw_through_soft_materials_without_absorbing_other_sprites() {
    let (mut harness, _) = run();
    // Only this test's lamps glow.
    let world = harness.world_mut();
    let owners: Vec<Entity> = world.query::<(Entity, &Glow)>().map(|(e, _)| e).collect();
    for owner in owners {
        world.remove_component::<Glow>(owner);
    }
    let halo = world.resource::<MaterialRegistry>().get(HALO_MATERIAL);
    assert!(halo.is_some(), "the manifest's soft halo");
    // Both sprites on one atlas page, as packed sprites are.
    let assets = world.resource_mut::<AssetRegistry>();
    mock_sprite(assets, "ex10_halo", 7, false);
    mock_sprite(assets, "t_lantern", 7, false);
    let at = world.resource::<CameraState>().position + Vec2::new(200.0, 150.0);
    for x in [0.0, 80.0] {
        let glow = Glow {
            offset: [0.0, 0.0],
            radius: 40.0,
            color: [255, 190, 100, 255],
            light: 0.8,
            core: 0.0,
            reach: None,
            hidden: false,
        };
        let lamp = (Transform::from_position(at + Vec2::X * x), glow);
        world.spawn_with((lamp.0, lamp.1, SceneEntity { state_id: GAMEPLAY }));
    }
    let lantern = world
        .resource::<AssetRegistry>()
        .sprite_id("t_lantern")
        .unwrap();
    world.spawn_with((
        Transform::from_position(at),
        Sprite::new(lantern),
        Visibility::default(),
    ));
    harness.step(1);
    let batches = extract_sprites_default(harness.world());
    let glows: Vec<_> = batches.iter().filter(|b| b.material_id == halo).collect();
    assert_eq!(glows.len(), 1);
    assert_eq!(glows[0].instances.len(), 2);
    let plain: Vec<_> = batches
        .iter()
        .filter(|b| b.material_id.is_none() && b.texture.0 == 7)
        .collect();
    assert!(!plain.is_empty(), "the lantern draws on its own");
    assert!(
        plain
            .iter()
            .all(|b| b.instances.iter().all(|i| i.size == [64.0, 64.0]))
    );
}

#[test]
fn cloud_parallax_and_vortex_instances_animate_without_world_mutation() {
    let (mut harness, _) = run();
    let world = harness.world_mut();
    let assets = world.resource_mut::<AssetRegistry>();
    for (i, name) in [
        "ex10_clouds_far",
        "ex10_vortex",
        "ex10_vortex_core",
        "ex10_infall_streak",
    ]
    .iter()
    .enumerate()
    {
        mock_sprite(assets, name, 300 + i as u32, false);
    }
    let centre = world.resource::<CameraState>().position + Vec2::new(300.0, 200.0);
    let hole = toys::place_black_hole(world, centre);
    world.get_mut::<BlackHole>(hole).unwrap().left = 2.0;
    world.insert_resource(SceneTime(0.0));
    let first = draw::backdrop(world);
    let still = toys::draw::effects(world);
    world.insert_resource(SceneTime(0.25));
    let second = draw::backdrop(world);
    let turned = toys::draw::effects(world);
    assert_ne!(
        first[0].instances[0].position, second[0].instances[0].position,
        "the clouds drift"
    );
    assert!(
        turned
            .iter()
            .flat_map(|b| &b.instances)
            .any(|i| i.rotation.abs() > 0.5)
    );
    let rotations = |batches: &[tungsten::render::SpriteBatch]| -> Vec<f32> {
        batches
            .iter()
            .flat_map(|b| b.instances.iter().map(|i| i.rotation))
            .collect()
    };
    assert_ne!(rotations(&still), rotations(&turned));
    assert_eq!(
        world.get::<Transform>(hole).unwrap().position,
        centre,
        "drawing moves nothing"
    );
    assert_eq!(world.query::<&BlackHole>().count(), 1);
}
