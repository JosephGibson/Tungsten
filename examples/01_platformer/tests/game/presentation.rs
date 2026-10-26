//! What draws: the wizard's clips and bursts, the ambient emitters and
//! the particle caps, the drawing order, the lit frames' feet, the props'
//! animations in step, the staff light's glow and light, the wizard's
//! overlays on his body, midnight's post stack and moon, the glows'
//! material batches, an enemy's alert and search marks with the alert's
//! one sting, F1's hit boxes beside the colliders while they land, the
//! theme's own colours' contrast, and the title's starry night, whose stars
//! leave with it.

use std::path::PathBuf;

use example_01_platformer::features::combat::draw::overlays;
use example_01_platformer::features::level::components::{AmbientEmitter, Glow, GlowParts};
use example_01_platformer::features::level::draw;
use example_01_platformer::features::level::systems::{HALO, HALO_MATERIAL};
use example_01_platformer::features::player::components::{Lantern, LanternAnchors, Moves};
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

use example_01_platformer::features::audio::components::{Sound, Sounded};
use example_01_platformer::features::enemies::beetle::{self, PLOUGH_HALF};
use example_01_platformer::features::enemies::components::{
    Brain, Enemy, EnemyKind, EnemyState, Strike,
};
use example_01_platformer::features::enemies::draw::{
    HIT_COLOR, HIT_THICKNESS, MARK_ALERT, MARK_SEARCH, mark,
};
use example_01_platformer::features::enemies::goblin::{self, STAB_HALF};
use example_01_platformer::features::enemies::troll::{self, SLAM_REACH, SWIPE_HALF};
use example_01_platformer::features::enemies::{self};
use example_01_platformer::features::level::components::SceneTime;
use example_01_platformer::features::toys::components::{OpeningOf, Ring, ShockRing};
use example_01_platformer::features::toys::{self, components::BlackHole};
use example_01_platformer::save::{self, SaveGame};
use example_01_platformer::shared::{SaveId, SaveIds};
use example_01_platformer::shell::{self as game_shell, Difficulty, RunStart};
use tungsten::PhysicsDebugOverlay;
use tungsten::core::Shape;
use tungsten::core::{AudioCommand, SoundRegistry, Time};
use tungsten::kit::{Hitbox, KitDebug, StateMachine};

use super::{aim_at, centre, choose, fall_dead, mock_sprite, pin_camera, press, run, teleport};

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
    // A black hole's layers, but its halo, which the moon's glow shares.
    for name in [
        "ex10_accretion_disk",
        "ex10_vortex",
        "ex10_infall_streak",
        "ex10_photon_ring",
        "ex10_vortex_core",
    ] {
        mock_sprite(assets, name, 510, false);
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
    toys::place_black_hole(world, far + Vec2::splat(100.0));
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
    let mut handles: Vec<u32> = batches
        .iter()
        .map(|batch| batch.texture.0)
        .filter(|handle| (501..=510).contains(handle))
        .collect();
    handles.dedup();
    // The sky, the tiles by layer, the hole at their far plane, the sprites
    // by depth, the cursor last.
    assert_eq!(handles, [501, 502, 503, 504, 505, 510, 506, 507, 508, 509]);
    let hole: Vec<f32> = batches
        .iter()
        .filter(|batch| batch.texture.0 == 510)
        .flat_map(|batch| batch.instances.iter().map(|quad| quad.z_norm))
        .collect();
    assert!(
        !hole.is_empty() && hole.iter().all(|&z| z == 1.0),
        "{hole:?}"
    );
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
        // The art's feet, its row 93, stand on the body's bottom.
        let row = BODY_OFFSET.y + PLAYER_HALF.y;
        assert!((sprite.position[1] + row - feet.y).abs() < 1e-3, "{name}");
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
    // A run starts dark: the first press lights it.
    assert!(harness.world().get::<Glow>(player).unwrap().hidden);
    harness.harness_mut().press_action("lantern");
    harness.step(1);
    harness.harness_mut().release_action("lantern");
    let world = harness.world();
    let anchors = world
        .resource::<DataRegistry>()
        .get::<LanternAnchors>("player")
        .unwrap()
        .clone();
    // A left-facing frame's orb mirrors its right-facing twin's about the
    // body's centre line, x 48 of the frame.
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
    assert!(
        harness
            .world()
            .get::<Light>(parts.light.expect("its light"))
            .unwrap()
            .intensity
            > 0.0
    );
    harness.harness_mut().press_action("lantern");
    harness.step(1);
    let world = harness.world();
    assert!(world.get::<Glow>(player).unwrap().hidden);
    assert_eq!(
        world
            .get::<Light>(parts.light.expect("its light"))
            .unwrap()
            .intensity,
        0.0
    );
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
fn the_wizards_overlays_sit_on_his_body() {
    let (mut harness, player) = run();
    harness.step(10);
    super::combat::apply(&mut harness, player, "burning", None);
    harness.step(1);
    let assets = harness.world_mut().resource_mut::<AssetRegistry>();
    mock_sprite(assets, HALO, 950, false);
    // The burning glow's halo, by its quad's centre.
    let halos: Vec<Vec2> = overlays(harness.world())
        .into_iter()
        .filter(|batch| batch.texture.0 == 950)
        .flat_map(|batch| batch.instances)
        .map(|quad| Vec2::from(quad.position) + Vec2::from(quad.size) * 0.5)
        .collect();
    let body = centre(&harness, player);
    assert!(
        halos.iter().any(|halo| halo.distance(body) < 0.5),
        "{halos:?} against his body at {body}"
    );
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
    fall_dead(&mut harness);
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
fn the_bloom_option_hides_the_halos_and_the_moons_glow() {
    let (mut harness, player) = run();
    // Lit, so its halo and core draw.
    harness.world_mut().get_mut::<Lantern>(player).unwrap().on = true;
    let assets = harness.world_mut().resource_mut::<AssetRegistry>();
    mock_sprite(assets, "ex10_sky", 700, false);
    mock_sprite(assets, draw::MOON, 701, false);
    mock_sprite(assets, "ex10_halo", 702, false);
    mock_sprite(assets, "ex10_flame_glow", 703, false);
    let shown = |harness: &mut UiHarness, on: bool| {
        harness.world_mut().resource_mut::<GameOptions>().bloom = on;
        harness.step(1);
        let world = harness.world();
        let parts = *world.get::<GlowParts>(player).unwrap();
        let visible =
            |part: Option<Entity>| world.get::<Visibility>(part.unwrap()).unwrap().visible;
        let moon: Vec<u32> = draw::backdrop(world)
            .iter()
            .map(|batch| batch.texture.0)
            .filter(|texture| (701..=703).contains(texture))
            .collect();
        (visible(parts.halo), visible(parts.core), moon)
    };
    assert_eq!(shown(&mut harness, true), (true, true, vec![702, 703, 701]));
    assert_eq!(
        shown(&mut harness, false),
        (false, true, vec![701]),
        "the halos and the moon's glow go, the flame's core and the moon stay"
    );
}

#[test]
fn midnight_uses_restrained_stock_post_passes_and_round_moon_at_all_aspects() {
    let (mut harness, _) = run();
    let world = harness.world_mut();
    let stack = world.resource::<PostStack>();
    assert!(
        matches!(stack.0.as_slice(), [PostPass::Bloom(b), PostPass::Vignette(v)] if b.intensity <= 0.7 && v.strength <= 0.2),
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
    let still = toys::draw::vortices(world);
    world.insert_resource(SceneTime(0.25));
    let second = draw::backdrop(world);
    let turned = toys::draw::vortices(world);
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

#[test]
fn the_backdrop_skips_quads_opaque_tiles_cover() {
    // Inside the Ember hollow the cave's opaque backs and walls cover most
    // of the backdrop, which draws nothing there; in the clearing's sky the
    // sky shows; and a game whose assets hot-reload culls nothing.
    use example_01_platformer::features::level::TILE;
    use example_01_platformer::features::level::components::AssetsReload;
    let (mut harness, _) = run();
    let world = harness.world_mut();
    let assets = world.resource_mut::<AssetRegistry>();
    let names = [
        "ex10_sky",
        draw::SKY_BLACK,
        "ex10_clouds_far",
        "ex10_clouds_near",
        "ex10_distant_ridges",
        "ex10_near_woodland",
    ];
    for (i, name) in names.iter().enumerate() {
        mock_sprite(assets, name, 800 + i as u32, false);
    }
    world.insert_resource(WindowSize {
        width: 1920,
        height: 1080,
    });
    let quads = |world: &tungsten::core::World| -> usize {
        draw::backdrop(world)
            .iter()
            .map(|b| b.instances.len())
            .sum()
    };
    let mut seen = Vec::new();
    for corner in [Vec2::new(240.0, 46.0), Vec2::new(10.0, 0.0)] {
        world.resource_mut::<CameraState>().position = corner * TILE;
        let culled = quads(world);
        world.insert_resource(AssetsReload);
        let all = quads(world);
        world.remove_resource::<AssetsReload>();
        seen.push((culled, all));
    }
    let [(hollow, hollow_all), (sky, sky_all)] = seen[..] else {
        unreachable!()
    };
    assert!(
        hollow < hollow_all,
        "the hollow culls: {hollow} of {hollow_all}"
    );
    assert!(
        sky > 0 && sky <= sky_all,
        "the sky shows: {sky} of {sky_all}"
    );
    world.resource_mut::<CameraState>().position = Vec2::new(10.0, 0.0) * TILE;
    let batches = draw::backdrop(world);
    assert!(
        batches
            .iter()
            .any(|b| b.texture.0 == 800 && !b.instances.is_empty()),
        "the clearing's sky quad is drawn"
    );
}

#[test]
fn spray_particles_draw_over_the_enemies() {
    use example_01_platformer::features::toys::components::Spray;
    use example_01_platformer::features::toys::systems::SPRAY_Z;
    for (slot, name) in [(1, "the flamethrower"), (2, "the ice spray")] {
        let (mut harness, player) = run();
        super::calm(&mut harness);
        super::ground(&mut harness, 1000.0);
        super::stand(&mut harness, player, 600.0, 1000.0);
        harness.harness_mut().set_dt(1.0 / 60.0);
        // Iron in reach, so the contact's emitter emits as well.
        let iron = Vec2::new(810.0, 940.0);
        toys::place_iron(harness.world_mut(), iron);
        harness.step(5);
        super::pin_camera(&mut harness, Vec2::new(200.0, 500.0));
        super::aim_at(&mut harness, iron);
        super::choose(&mut harness, slot);
        harness.harness_mut().press_action("cast");
        let mut met = [false; 3];
        for _ in 0..30 {
            harness.step(1);
            let world = harness.world();
            let Some(spray) = world.query::<&Spray>().next().copied() else {
                continue;
            };
            for (particle, sprite) in world.query::<(&Particle, &Sprite)>() {
                let from = spray
                    .emitters
                    .iter()
                    .position(|emitter| emitter.is_some() && *emitter == particle.emitter);
                if let (Some(index), true) = (from, particle.age > 0.0) {
                    assert_eq!(sprite.z_order, SPRAY_Z, "{name}, emitter {index}");
                    met[index] = true;
                }
            }
        }
        harness.harness_mut().release_action("cast");
        assert_eq!(met, [true; 3], "{name}: every emitter's particles met");
    }
}

#[test]
fn a_death_at_the_burst_cap_takes_a_cosmetic_bursts_slot() {
    use example_01_platformer::features::dummy;
    let (mut harness, _) = run();
    super::calm(&mut harness);
    let target = dummy::spawn(harness.world_mut(), Vec2::new(868.0, 600.0));
    harness.step(2);
    let world = harness.world_mut();
    let counted = |world: &tungsten::core::World| {
        world
            .query::<&Burst>()
            .filter(|burst| burst.counted)
            .count()
    };
    while counted(world) < BURST_CAP {
        spawn_burst(world, "ex10_landing_dust", Vec2::new(500.0, 500.0)).expect("a slot");
    }
    assert!(spawn_burst(world, "ex10_landing_dust", Vec2::new(500.0, 500.0)).is_none());
    let oldest = world
        .query::<(Entity, &Burst)>()
        .filter(|(_, burst)| !burst.key)
        .min_by_key(|(_, burst)| burst.order)
        .map(|(entity, _)| entity)
        .unwrap();
    damage(world, target, 10_000.0);
    harness.step(1);
    let world = harness.world();
    let poof = world
        .resource::<ParticleConfigRegistry>()
        .id_for_name("ex10_poof");
    assert!(
        world
            .query::<(&Burst, &ParticleEmitter)>()
            .any(|(burst, emitter)| burst.key && Some(emitter.config) == poof),
        "its death's poof"
    );
    assert_eq!(counted(world), BURST_CAP);
    assert!(
        !world.get::<Burst>(oldest).unwrap().counted,
        "gave its slot"
    );
    assert!(world.get::<ParticleEmitterState>(oldest).unwrap().drained);
}

/// A crawler on the sky's ground with its centre at `x`, at Normal.
fn crawler(harness: &mut UiHarness, x: f32) -> Entity {
    let kind = EnemyKind::Crawler;
    let centre = Vec2::new(x, SKY_TOP - kind.info().half.y - 0.5);
    let world = harness.world_mut();
    let id = world.resource_mut::<SaveIds>().take();
    let crawler = enemies::spawn(world, kind, centre - kind.info().centre, id).expect("its prefab");
    enemies::scale(world, crawler, Difficulty::Normal);
    crawler
}

/// The sting's plays in the last frame.
fn stings(harness: &UiHarness) -> usize {
    let sting = harness
        .world()
        .resource::<SoundRegistry>()
        .get_by_id("ex10_enemy_spotted_sfx");
    harness
        .harness()
        .audio()
        .iter()
        .filter(|command| matches!(command, AudioCommand::Play { handle, .. } if Some(*handle) == sting))
        .count()
}

/// Holds the courier on its spot at `x`, whole, and steps a frame.
fn hold_step(harness: &mut UiHarness, player: Entity, x: f32) {
    super::stand(harness, player, x, SKY_TOP);
    tungsten::kit::heal(harness.world_mut(), player, 100.0);
    harness.step(1);
}

fn machine_state(harness: &UiHarness, enemy: Entity) -> EnemyState {
    harness
        .world()
        .get::<StateMachine<EnemyState>>(enemy)
        .expect("a machine")
        .state()
}

/// The sky's ground the enemies' tests stand on.
const SKY_TOP: f32 = 1000.0;

/// An enemy of `kind` on the sky's ground at `x`, at Normal, its mind
/// taken, so it stands.
fn standing(harness: &mut UiHarness, kind: EnemyKind, x: f32) -> Entity {
    let centre = Vec2::new(x, SKY_TOP - kind.info().half.y - 0.5);
    let world = harness.world_mut();
    let id = world.resource_mut::<SaveIds>().take();
    let enemy = enemies::spawn(world, kind, centre - kind.info().centre, id).expect("its prefab");
    enemies::scale(world, enemy, Difficulty::Normal);
    world.remove_component::<StateMachine<EnemyState>>(enemy);
    enemy
}

/// Puts `enemy` in its attack with `strike` as the blow it lands.
fn landing(harness: &mut UiHarness, enemy: Entity, strike: Strike) {
    let world = harness.world_mut();
    world.get_mut::<Brain>(enemy).unwrap().strike = Some(strike);
    world.insert(enemy, StateMachine::new(EnemyState::Attack));
}

#[test]
fn f1_draws_the_hit_boxes_with_the_colliders_while_they_land() {
    let mut harness = super::harness();
    super::start(&mut harness);
    let player = super::player_entity(&harness);
    super::calm(&mut harness);
    harness.harness_mut().set_dt(1.0 / 60.0);
    super::ground(&mut harness, SKY_TOP);
    hold_step(&mut harness, player, 300.0);
    let goblin = standing(&mut harness, EnemyKind::Goblin, 900.0);
    let swiper = standing(&mut harness, EnemyKind::Troll, 1300.0);
    let bat = standing(&mut harness, EnemyKind::Bat, 1700.0);
    let slammer = standing(&mut harness, EnemyKind::Troll, 2150.0);
    harness.step(1);
    let hit_shapes = |harness: &UiHarness| {
        let draw = harness.harness().draw().expect("a frame drew");
        let quads = draw
            .debug_quads
            .iter()
            .filter(|q| q.color == HIT_COLOR)
            .count();
        let lines = draw
            .debug_lines
            .iter()
            .filter(|l| l.color == HIT_COLOR)
            .count();
        quads + lines
    };
    harness
        .world_mut()
        .resource_mut::<PhysicsDebugOverlay>()
        .enabled = true;
    harness.step(2);
    assert!(
        harness.world().resource::<KitDebug>().enabled(),
        "F1 turns the kit's on"
    );
    assert_eq!(
        hit_shapes(&harness),
        0,
        "no blow lands, the bat's window shut"
    );
    // A stab, a swipe and a slam landing, and the bat's dive window open.
    landing(&mut harness, goblin, Strike::Stab);
    landing(&mut harness, swiper, Strike::Swipe);
    landing(&mut harness, slammer, Strike::Slam);
    harness.world_mut().get_mut::<Hitbox>(bat).unwrap().arm();
    harness.step(2);
    let world = harness.world();
    let draw = harness.harness().draw().expect("a frame drew");
    let quad_at = |corner: Vec2| {
        draw.debug_quads
            .iter()
            .find(|quad| Vec2::from(quad.position).distance(corner) < 0.01)
            .map(|quad| (quad.color, quad.size[1]))
    };
    let hit = Some((HIT_COLOR, HIT_THICKNESS));
    let hitbox = world.get::<Hitbox>(bat).unwrap();
    let Shape::Aabb { half_extents } = hitbox.shape().shape() else {
        unreachable!("the bat's hitbox is a box")
    };
    let at = world.get::<Position>(bat).unwrap().0 + hitbox.offset();
    assert_eq!(quad_at(at - half_extents), hit, "the kit's armed hitbox");
    let stab = goblin::stab_box(world, goblin).unwrap();
    assert_eq!(quad_at(stab - STAB_HALF), hit, "the stab's box");
    let swipe = troll::swipe_box(world, swiper).unwrap();
    assert_eq!(quad_at(swipe - SWIPE_HALF), hit, "the swipe's box");
    let fist = troll::fist(world, slammer).unwrap();
    let slam = draw.debug_lines.iter().any(|line| {
        line.color == HIT_COLOR
            && line.thickness == HIT_THICKNESS
            && (Vec2::from(line.a).distance(fist) - SLAM_REACH).abs() < 0.5
    });
    assert!(slam, "the slam's reach about its fist");
    let swipe_fist = troll::fist(world, swiper).unwrap();
    let swiper_slams = draw.debug_lines.iter().any(|line| {
        line.color == HIT_COLOR
            && (Vec2::from(line.a).distance(swipe_fist) - SLAM_REACH).abs() < 0.5
    });
    assert!(!swiper_slams, "a swipe shows no slam");
    // F1 off: the kit's off and nothing draws in the hit colour.
    harness
        .world_mut()
        .resource_mut::<PhysicsDebugOverlay>()
        .enabled = false;
    harness.step(2);
    assert!(!harness.world().resource::<KitDebug>().enabled());
    assert_eq!(hit_shapes(&harness), 0, "no hit shape");
}

#[test]
fn f1_outlines_a_charging_beetles_plough_box() {
    let mut harness = super::harness();
    super::start(&mut harness);
    let player = super::player_entity(&harness);
    super::calm(&mut harness);
    harness.harness_mut().set_dt(1.0 / 60.0);
    super::ground(&mut harness, SKY_TOP);
    hold_step(&mut harness, player, 300.0);
    let beetle = standing(&mut harness, EnemyKind::Beetle, 1200.0);
    harness
        .world_mut()
        .resource_mut::<PhysicsDebugOverlay>()
        .enabled = true;
    harness.step(2);
    let plough_quad = |harness: &UiHarness| {
        let corner = beetle::plough_box(harness.world(), beetle).unwrap() - PLOUGH_HALF;
        let draw = harness.harness().draw().expect("a frame drew");
        draw.debug_quads
            .iter()
            .find(|quad| Vec2::from(quad.position).distance(corner) < 0.01)
            .map(|quad| (quad.color, quad.size[1]))
    };
    assert_eq!(plough_quad(&harness), None, "it stands: no plough");
    let world = harness.world_mut();
    world.get_mut::<Enemy>(beetle).unwrap().heading = 1.0;
    world.insert(beetle, StateMachine::new(EnemyState::Attack));
    harness.step(2);
    assert_eq!(
        plough_quad(&harness),
        Some((HIT_COLOR, HIT_THICKNESS)),
        "the plough's box while it charges"
    );
}

#[test]
fn an_alert_shows_its_mark_and_stings_once() {
    let mut harness = super::harness();
    super::start(&mut harness);
    let player = super::player_entity(&harness);
    super::calm(&mut harness);
    harness.harness_mut().set_dt(1.0 / 60.0);
    super::ground(&mut harness, SKY_TOP);
    hold_step(&mut harness, player, 300.0);
    super::pin_camera(&mut harness, Vec2::new(0.0, 400.0));
    let spotter = crawler(&mut harness, 600.0);
    let neighbour = crawler(&mut harness, 640.0);
    // Both come alert in one step: one sting for the two.
    let (mut heard, mut sounded) = (0, 0);
    let mut alerted = false;
    for _ in 0..60 {
        hold_step(&mut harness, player, 300.0);
        heard += stings(&harness);
        sounded += harness
            .harness()
            .events::<Sounded>()
            .filter(|s| s.sound == Sound::Alerted)
            .count();
        if mark(harness.world(), spotter) == Some(MARK_ALERT) {
            alerted = true;
            break;
        }
    }
    assert!(alerted, "a fresh alert shows its !");
    assert_eq!(
        mark(harness.world(), neighbour),
        Some(MARK_ALERT),
        "its neighbour's too"
    );
    assert_eq!((sounded, heard), (2, 1), "two alerts, one sting");
    // The ! shows past the alert into the chase; a save 0.5 s in, loaded
    // through JSON, shows it 0.3 s more.
    for _ in 0..30 {
        hold_step(&mut harness, player, 300.0);
        heard += stings(&harness);
        assert_eq!(mark(harness.world(), spotter), Some(MARK_ALERT));
    }
    assert_ne!(
        machine_state(&harness, spotter),
        EnemyState::Alert,
        "past its alert"
    );
    let saved = save::capture(harness.world()).expect("a live run captures");
    let loaded: SaveGame =
        serde_json::from_value(serde_json::to_value(&saved).unwrap()).expect("it loads");
    let mut fresh = super::harness();
    fresh.world_mut().resource_mut::<Time>().pause();
    game_shell::start_run(fresh.world_mut(), RunStart::from(loaded), None);
    super::settle(&mut fresh);
    fresh.step(1);
    fresh.harness_mut().set_dt(1.0 / 60.0);
    fresh.world_mut().resource_mut::<Time>().resume();
    let restored = fresh
        .world()
        .query::<(Entity, &SaveId)>()
        .find(|(_, id)| Some(**id) == harness.world().get::<SaveId>(spotter).copied())
        .map(|(entity, _)| entity)
        .expect("the spotter loaded");
    let mut more: u32 = 0;
    for _ in 0..40 {
        if mark(fresh.world(), restored) != Some(MARK_ALERT) {
            break;
        }
        fresh.step(1);
        more += super::steps_this_frame(&fresh);
    }
    assert!(more.abs_diff(18) <= 1, "0.3 s more: {more} steps");
    // The ! runs out; then out of sight, short of dormancy from where its
    // lunge left it, it searches: a ? past its search's first 0.4 s.
    for _ in 0..20 {
        hold_step(&mut harness, player, 300.0);
        heard += stings(&harness);
    }
    assert_ne!(
        mark(harness.world(), spotter),
        Some(MARK_ALERT),
        "its ! ran out"
    );
    let mut searching = false;
    for _ in 0..240 {
        hold_step(&mut harness, player, 1400.0);
        if machine_state(&harness, spotter) == EnemyState::Search {
            searching = true;
            break;
        }
    }
    assert!(searching, "it searches");
    assert_eq!(mark(harness.world(), spotter), None, "no ? at once");
    let mut asked = false;
    for _ in 0..40 {
        hold_step(&mut harness, player, 1400.0);
        asked |= mark(harness.world(), spotter) == Some(MARK_SEARCH);
    }
    assert!(asked, "a ? past 0.4 s");
    // Seen again from the search: no sting and no !.
    let x = enemies::centre(harness.world(), spotter).unwrap().x - 200.0;
    let mut again = false;
    for _ in 0..60 {
        hold_step(&mut harness, player, x);
        heard += stings(&harness);
        assert_ne!(
            mark(harness.world(), spotter),
            Some(MARK_ALERT),
            "no ! from a search"
        );
        if machine_state(&harness, spotter) == EnemyState::Alert {
            again = true;
            break;
        }
    }
    assert!(again, "alert again");
    assert_eq!(heard, 1, "no sting from a search");
}

/// WCAG's contrast ratio of two opaque sRGB colours, as the template's
/// shell test computes it.
fn contrast(a: [u8; 4], b: [u8; 4]) -> f32 {
    let luminance = |c: [u8; 4]| {
        let channel = |v: u8| {
            let v = f32::from(v) / 255.0;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(c[0]) + 0.7152 * channel(c[1]) + 0.0722 * channel(c[2])
    };
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

#[test]
fn the_theme_reads_in_its_own_colours() {
    // 01's theme, the template's structure in 01's colours, holds its own
    // floors over the template's ten pairs, each under its reading.
    let theme = tungsten::core::ui::ThemeFile::load("assets/ui/themes/shell.json")
        .expect("the theme parses")
        .into_theme();
    let colors = theme.tokens.colors;
    let surface = colors.surface;
    for (name, color, against, least) in [
        ("text", colors.text, surface, 16.0),
        ("text_muted", colors.text_muted, surface, 8.3),
        ("accent", colors.accent, surface, 5.5),
        ("accent_text", colors.accent_text, colors.accent, 5.8),
        ("focus", colors.focus, surface, 10.2),
        ("border", colors.border, surface, 3.4),
        ("disabled_text", colors.disabled_text, surface, 3.8),
        ("danger", colors.danger, surface, 6.9),
        (
            "danger on the background",
            colors.danger,
            colors.background,
            7.2,
        ),
        (
            "danger on a hovered fill",
            colors.danger,
            [0x11, 0x14, 0x2a, 255],
            6.4,
        ),
    ] {
        let ratio = contrast(color, against);
        assert!(ratio >= least - 0.01, "{name}: {ratio:.2} < {least}");
        assert_eq!(color[3], 255, "{name} is opaque");
    }
}

#[test]
fn the_title_draws_its_sky_and_its_twinkles_leave_with_it() {
    use example_01_platformer::features::level::title::{TITLE_STAR, TWINKLE_SIZES, TWINKLES};
    use example_01_platformer::shell::title::TITLE;
    let mut harness = super::harness();
    // The backdrop's sprites, as the harness loads none: the midnight sky,
    // the black night sky and the clouds.
    let assets = harness.world_mut().resource_mut::<AssetRegistry>();
    let sprites = [
        "ex10_sky",
        "ex10_sky_black",
        "ex10_clouds_far",
        "ex10_clouds_near",
    ];
    for (i, name) in sprites.into_iter().enumerate() {
        mock_sprite(assets, name, 960 + i as u32, false);
    }
    harness.step(2);
    let drawn = |h: &UiHarness| -> Vec<u32> {
        draw::backdrop(h.world())
            .iter()
            .map(|batch| batch.texture.0)
            .collect()
    };
    let title = drawn(&harness);
    assert!(title.contains(&961), "the black night sky: {title:?}");
    assert!(
        !title.contains(&962) && !title.contains(&963),
        "no clouds: {title:?}"
    );
    let configs = harness.world().resource::<ParticleConfigRegistry>();
    let sizes: Vec<_> = TWINKLE_SIZES
        .iter()
        .map(|(name, _)| configs.id_for_name(name).expect("its config"))
        .collect();
    let twinkles = |h: &UiHarness| -> Vec<Option<SceneEntity>> {
        h.world()
            .query::<(&ParticleEmitter, Option<&SceneEntity>)>()
            .filter(|(emitter, _)| sizes.contains(&emitter.config))
            .map(|(_, owner)| owner.copied())
            .collect()
    };
    let owners = twinkles(&harness);
    assert_eq!(owners.len(), TWINKLES as usize);
    assert!(
        owners
            .iter()
            .all(|owner| owner.is_some_and(|owner| owner.state_id == TITLE)),
        "the title's"
    );
    let stars = |h: &UiHarness| {
        h.world()
            .query::<&Particle>()
            .filter(|particle| particle.config.sprite == TITLE_STAR)
            .count()
    };
    let mut twinkling = false;
    for _ in 0..240 {
        harness.step(1);
        if stars(&harness) > 0 {
            twinkling = true;
            break;
        }
    }
    assert!(twinkling, "the stars twinkle");
    // A New Game through the curtain: no star stays, and a run's sky draws.
    super::start(&mut harness);
    harness.step(1);
    assert!(twinkles(&harness).is_empty(), "the twinkles left");
    assert_eq!(stars(&harness), 0, "no star lingers");
    let run = drawn(&harness);
    assert!(run.contains(&960), "the run's sky: {run:?}");
}

#[test]
fn a_hole_opens_over_three_tenths_of_a_second() {
    let (mut harness, _) = run();
    harness.harness_mut().set_dt(1.0 / 60.0);
    let assets = harness.world_mut().resource_mut::<AssetRegistry>();
    for name in [
        "ex10_accretion_disk",
        "ex10_vortex",
        "ex10_infall_streak",
        "ex10_photon_ring",
        "ex10_vortex_core",
    ] {
        mock_sprite(assets, name, 510, false);
    }
    // Slot 6's press opens one at the cursor, in the sky.
    pin_camera(&mut harness, Vec2::new(200.0, 200.0));
    aim_at(&mut harness, Vec2::new(520.0, 480.0));
    choose(&mut harness, 5);
    harness.harness_mut().press_action("cast");
    harness.step(1);
    harness.harness_mut().release_action("cast");
    let world = harness.world_mut();
    let hole = world
        .query::<(Entity, &BlackHole)>()
        .map(|(entity, _)| entity)
        .next()
        .expect("the press opened a hole");
    let opening: Vec<f32> = world
        .query::<&ShockRing>()
        .filter(|ring| ring.kind == Ring::Open)
        .map(|ring| ring.size)
        .collect();
    assert!(
        opening.len() == 1 && (opening[0] - 432.0).abs() < 0.01,
        "its closing ring, 432 px across: {opening:?}"
    );
    let rushes = world
        .query::<(&OpeningOf, &ParticleEmitter)>()
        .filter(|(of, _)| of.0 == hole)
        .count();
    assert_eq!(rushes, 1, "the rush's emitter names the hole");
    // The vortex at its age 0 and at 0.3 s, at the same drawn time.
    let sizes = |world: &mut tungsten::core::World, age: f32| {
        world.get_mut::<BlackHole>(hole).unwrap().left = toys::HOLE_SECS - age;
        toys::draw::vortices(world)
            .iter()
            .flat_map(|batch| &batch.instances)
            .map(|quad| quad.size[0])
            .collect::<Vec<f32>>()
    };
    let (born, open) = (sizes(world, 0.0), sizes(world, toys::HOLE_OPEN_SECS));
    assert!(!born.is_empty() && born.len() == open.len());
    for (small, full) in born.iter().zip(&open) {
        assert!(
            (small / full - 0.08).abs() <= 0.01,
            "{small} of {full}: not 0.08"
        );
    }
}
