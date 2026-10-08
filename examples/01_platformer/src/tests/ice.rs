use super::*;
use crate::brick::{
    BRICK_FRICTION, BRICK_HALF, BRICK_MASS, FROZEN_FRICTION, IronBrick, IronScrap, SCRAP_COUNT,
    SCRAP_HALF, brick_friction, spawn_brick,
};
use crate::fireball::{
    FIREBALL_BLAST_RADIUS, FIREBALL_SPEED, explode_fireball, fireball_flight_system, spawn_fireball,
};
use crate::ice::{
    Chill, FREEZE_TIME, Frozen, ICE_BEAM_RANGE, IceBeam, IcePulse, cast_ice_beam_system,
    freeze_amount, freeze_iron, ice_beam_cleanup, ice_beam_presentation,
};
use crate::state::{EffectSounds, TransientEmitter};
use tungsten::core::{Particle, ParticleBudget, ParticleConfigRegistry, ParticleEmitter};

fn aim_and_press(world: &mut World, at: Vec2) {
    crate::setup::platformer_bindings(world);
    let camera = world.get_resource_mut::<CameraState>().unwrap();
    camera.position = Vec2::ZERO;
    camera.zoom = 1.0;
    let input = world.get_resource_mut::<InputState>().unwrap();
    input.update_cursor_position(at.x, at.y);
    input.key_down(KeyCode::KeyF);
}

fn spray_steps(world: &mut World, steps: usize) {
    for _ in 0..steps {
        cast_ice_beam_system(world);
    }
}

fn ice_sounds() -> EffectSounds {
    EffectSounds {
        cast: (AudioHandle(1), 1.0),
        ice_cast: (AudioHandle(6), 0.42),
        ice_loop: (AudioHandle(7), 0.32),
        ice_freeze: (AudioHandle(8), 0.55),
        ice_end: (AudioHandle(10), 0.22),
        ice_shatter: (AudioHandle(11), 0.5),
        blast: (AudioHandle(2), 1.0),
        extinguish: (AudioHandle(3), 1.0),
        hit: (AudioHandle(4), 1.0),
        crush: (AudioHandle(5), 1.0),
        extinguish_cooldown: 0.0,
        crush_cooldown: 0.0,
    }
}

fn ice_harness() -> Harness {
    let plugins = DefaultPlugins::set()
        .without::<PhysicsPlugin>()
        .without::<GameFeelPlugin>()
        .without::<CameraPlugin>();
    let mut app = App::with_plugins(Config::default(), plugins).unwrap();
    seed(app.world_mut());
    app.add_system_to(
        tungsten::Stage::FixedUpdate,
        tungsten::system("cast_ice_beam_system", cast_ice_beam_system),
    );
    app.add_system_named("ice_beam_cleanup", ice_beam_cleanup);
    app.add_system_named(
        "transient_emitter_cleanup",
        crate::systems::transient_emitter_cleanup,
    );
    app.add_system_to(
        tungsten::Stage::PostUpdate,
        tungsten::system("ice_beam_presentation", ice_beam_presentation)
            .before(tungsten::plugins::PARTICLE_COUNT_REFRESH),
    );
    Harness::new(app)
}

#[test]
fn frozen_iron_and_scraps_render_and_escaped_scraps_are_cleaned_up() {
    let mut world = seed_world();
    load_presentation_assets(&mut world);
    let mut assets = tungsten::core::AssetRegistry::new();
    for (index, name) in [
        "ex10_iron_brick_big_0_0",
        "ex10_iron_brick_big_0_1",
        "ex10_iron_brick_big_1_0",
        "ex10_iron_brick_big_1_1",
    ]
    .into_iter()
    .enumerate()
    {
        mock_sprite(&mut assets, name, index as u32 + 1, true);
    }
    for (index, name) in [
        "ex10_iron_frost_big_0_0",
        "ex10_iron_frost_big_0_1",
        "ex10_iron_frost_big_1_0",
        "ex10_iron_frost_big_1_1",
    ]
    .into_iter()
    .enumerate()
    {
        mock_sprite(&mut assets, name, index as u32 + 5, false);
    }
    for (index, name) in [
        "ex10_iron_scrap_0",
        "ex10_iron_scrap_1",
        "ex10_iron_scrap_2",
        "ex10_iron_scrap_3",
    ]
    .into_iter()
    .enumerate()
    {
        mock_sprite(&mut assets, name, index as u32 + 9, true);
    }
    for (index, name) in [
        "ex10_iron_scrap_frost_0",
        "ex10_iron_scrap_frost_1",
        "ex10_iron_scrap_frost_2",
        "ex10_iron_scrap_frost_3",
    ]
    .into_iter()
    .enumerate()
    {
        mock_sprite(&mut assets, name, index as u32 + 13, false);
    }
    world.insert_resource(assets);
    let center = Vec2::new(200.0, 100.0);
    let brick = spawn_brick(&mut world, center);
    world.insert(brick, Chill { amount: 0.5 });
    let partial = crate::extract::extract_sprites(&world);
    assert!(
        partial
            .iter()
            .filter(|b| b.lit)
            .all(|b| { b.instances.iter().all(|i| i.color == [177, 225, 255, 255]) })
    );
    assert!(
        partial
            .iter()
            .filter(|b| !b.lit)
            .all(|b| { b.instances.iter().all(|i| i.color[3] == 127) })
    );
    freeze_iron(&mut world, brick);
    assert!(!world.has::<Chill>(brick));
    let batches = crate::extract::extract_sprites(&world);
    assert_eq!(batches.iter().map(|b| b.instances.len()).sum::<usize>(), 8);
    assert!(
        batches
            .iter()
            .filter(|b| b.lit)
            .all(|b| b.instances.iter().all(|i| i.color == [100, 195, 255, 255]))
    );
    assert_eq!(
        batches
            .iter()
            .filter(|b| !b.lit)
            .map(|b| b.instances.len())
            .sum::<usize>(),
        4,
        "the ice coating stays visible without scene lighting"
    );

    crate::brick::thermal_shatter(&mut world, center, FIREBALL_BLAST_RADIUS);
    let batches = crate::extract::extract_sprites(&world);
    assert_eq!(
        batches.iter().map(|b| b.instances.len()).sum::<usize>(),
        SCRAP_COUNT
    );
    assert!(batches.iter().all(|b| b.lit
        && (9..=12).contains(&b.texture.0)
        && b.instances.iter().all(|i| i.size == [32.0, 32.0])));
    world.insert_resource(crate::gameplay::SceneTime(0.25));
    let tumbling = crate::extract::extract_sprites(&world);
    assert!(batches.iter().zip(&tumbling).all(|(before, after)| {
        before
            .instances
            .iter()
            .zip(&after.instances)
            .all(|(a, b)| (a.rotation - b.rotation).abs() > 0.5)
    }));
    let paused = crate::extract::extract_sprites(&world);
    assert!(tumbling.iter().zip(&paused).all(|(a, b)| {
        a.instances
            .iter()
            .zip(&b.instances)
            .all(|(a, b)| a.rotation == b.rotation)
    }));
    let refrozen = world.query::<(Entity, &IronScrap)>().next().unwrap().0;
    freeze_iron(&mut world, refrozen);
    let refrozen_batches = crate::extract::extract_sprites(&world);
    let coating = refrozen_batches.iter().find(|b| !b.lit).unwrap().instances[0];
    let metal = refrozen_batches
        .iter()
        .filter(|b| b.lit)
        .flat_map(|b| &b.instances)
        .find(|i| i.position == coating.position)
        .unwrap();
    assert_eq!(
        metal.rotation, coating.rotation,
        "frost follows the tumbling metal"
    );
    assert_eq!(
        crate::extract::extract_sprites(&world)
            .iter()
            .filter(|b| !b.lit)
            .map(|b| b.instances.len())
            .sum::<usize>(),
        1
    );
    let escaped = world.query::<(Entity, &IronScrap)>().next().unwrap().0;
    world.get_mut::<Position>(escaped).unwrap().0 = WORLD_BOUNDS_MAX + Vec2::splat(100.0);
    despawn_out_of_bounds(&mut world);
    let commands = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush(commands);
    assert!(!world.is_alive(escaped));
    assert_eq!(
        world.query::<(Entity, &IronScrap)>().count(),
        SCRAP_COUNT - 1
    );
}

#[test]
fn holding_f_gradually_freezes_iron_with_particles_and_a_single_audio_loop() {
    let mut harness = ice_harness();
    let world = harness.world_mut();
    load_presentation_assets(world);
    spawn_test_player(world, Vec2::new(100.0, 100.0));
    let near = spawn_brick(world, Vec2::new(400.0, 100.0));
    let behind = spawn_brick(world, Vec2::new(650.0, 100.0));
    world.insert_resource(ice_sounds());
    aim_and_press(world, Vec2::new(650.0, 100.0));
    harness.step(1);
    let world = harness.world();
    assert!(!world.has::<Frozen>(near));
    assert!(freeze_amount(world, near) > 0.0);
    assert!(!world.has::<Frozen>(behind));
    let beam = world.query::<(Entity, &IceBeam)>().next().unwrap().1;
    assert!((beam.end.x - (400.0 - BRICK_HALF.x - 4.0)).abs() < 0.001);
    assert_eq!(world.query::<(Entity, &IceBeam)>().count(), 1);
    assert!(matches!(
        harness.audio(),
        [
            AudioCommand::Play {
                handle: AudioHandle(6),
                looping: false,
                ..
            },
            AudioCommand::Play {
                handle: AudioHandle(7),
                looping: true,
                ..
            }
        ]
    ));
    harness.step(44);
    assert!(harness.audio().is_empty());
    let world = harness.world_mut();
    assert!((freeze_amount(world, near) - 0.5).abs() < 0.001);
    assert!(!world.has::<Frozen>(near));
    crate::brick::thermal_shatter(world, Vec2::new(400.0, 100.0), FIREBALL_BLAST_RADIUS);
    assert!(world.is_alive(near), "partial frost is not brittle yet");
    assert!(world.query::<(Entity, &Particle)>().count() > 50);
    harness.step(44);
    assert!(!harness.world().has::<Frozen>(near));
    harness.step(1);
    assert!(harness.world().has::<Frozen>(near));
    assert!(!harness.world().has::<Frozen>(behind));
    assert!(matches!(
        harness.audio(),
        [AudioCommand::Play {
            handle: AudioHandle(8),
            looping: false,
            ..
        }]
    ));
    harness.step(60);
    assert!(
        harness.audio().is_empty(),
        "no repeated start or freeze sounds"
    );
    assert_eq!(harness.world().query::<(Entity, &IceBeam)>().count(), 1);
    input_mut(&mut harness).key_up(KeyCode::KeyF);
    harness.step(1);
    assert!(matches!(
        harness.audio(),
        [
            AudioCommand::Stop {
                handle: AudioHandle(7)
            },
            AudioCommand::Play {
                handle: AudioHandle(10),
                looping: false,
                ..
            }
        ]
    ));
    harness.step(120);
    let world = harness.world_mut();
    assert_eq!(world.query::<(Entity, &IceBeam)>().count(), 0);
    assert_eq!(world.query::<(Entity, &ParticleEmitter)>().count(), 0);
    assert_eq!(world.query::<(Entity, &Particle)>().count(), 0);
    cast_ice_beam_system(world);
    assert_eq!(
        world.query::<(Entity, &IceBeam)>().count(),
        0,
        "a released channel stays stopped"
    );
    world
        .get_resource_mut::<InputState>()
        .unwrap()
        .key_up(KeyCode::KeyF);
    aim_and_press(world, Vec2::new(650.0, 100.0));
    cast_ice_beam_system(world);
    assert_eq!(world.query::<(Entity, &IceBeam)>().count(), 1);
}

#[test]
fn beam_stops_on_colliders_and_tiles_and_has_a_finite_range() {
    for blocker in 0..3 {
        let mut world = seed_world();
        spawn_test_player(&mut world, Vec2::new(100.0, 100.0));
        let target = spawn_brick(&mut world, Vec2::new(500.0, 100.0));
        if blocker == 0 {
            world.spawn_with(RigidBodyBundle::r#static(
                Position(Vec2::new(250.0, 100.0)),
                Collider::aabb(Vec2::new(2.0, 80.0)),
            ));
        } else if blocker == 1 {
            let mut map = solid_floor(12);
            map.layers[0].tiles.fill(-1);
            map.layers[0].tiles[12 + 4] = 0;
            world
                .get_resource_mut::<TilemapRegistry>()
                .unwrap()
                .insert("wall".into(), map);
            let e = world.spawn();
            world.insert(e, TilemapInstance::new("wall", Vec2::ZERO));
        } else {
            world.get_mut::<Position>(target).unwrap().0.x =
                100.0 + ICE_BEAM_RANGE + BRICK_HALF.x + 10.0;
        }
        aim_and_press(&mut world, Vec2::new(1600.0, 100.0));
        spray_steps(&mut world, (FREEZE_TIME * 60.0) as usize);
        assert!(!world.has::<Frozen>(target), "blocker {blocker}");
        assert_eq!(freeze_amount(&world, target), 0.0, "blocker {blocker}");
    }
    // A round marble blocks the ray too, including a diagonal shot.
    let mut world = seed_world();
    spawn_test_player(&mut world, Vec2::new(100.0, 100.0));
    let brick = spawn_brick(&mut world, Vec2::new(400.0, 400.0));
    let ball = world.spawn_with(RigidBodyBundle::dynamic(
        Position(Vec2::splat(250.0)),
        Collider::circle(80.0),
    ));
    aim_and_press(&mut world, Vec2::splat(400.0));
    spray_steps(&mut world, (FREEZE_TIME * 60.0) as usize);
    assert!(!world.has::<Frozen>(brick));
    assert_eq!(freeze_amount(&world, brick), 0.0);
    assert!(!world.has::<Frozen>(ball));
}

#[test]
fn fireball_contact_shatters_frozen_iron_into_sixteen_physical_scraps() {
    let mut world = seed_world();
    spawn_test_player(&mut world, Vec2::new(100.0, 100.0));
    let at = Vec2::new(400.0, 100.0);
    let brick = spawn_brick(&mut world, at);
    aim_and_press(&mut world, at);
    spray_steps(&mut world, (FREEZE_TIME * 60.0) as usize);
    let missile = spawn_fireball(
        &mut world,
        Vec2::new(300.0, 100.0),
        Vec2::X * FIREBALL_SPEED,
    );
    for _ in 0..5 {
        fireball_flight_system(&mut world);
    }
    assert!(!world.is_alive(missile));
    assert!(!world.is_alive(brick));
    assert_eq!(world.query::<(Entity, &IronBrick)>().count(), 0);
    let scraps: Vec<_> = world
        .query::<(Entity, &IronScrap, &Position, &RigidBody)>()
        .collect();
    assert_eq!(scraps.len(), SCRAP_COUNT);
    let mass: f32 = scraps
        .iter()
        .map(|(_, _, _, body)| 1.0 / body.inv_mass)
        .sum();
    assert!((mass - BRICK_MASS).abs() < 0.001);
    for &(e, scrap, p, _) in &scraps {
        assert!(
            (1.0 / world.get::<RigidBody>(e).unwrap().inv_mass - BRICK_MASS / 16.0).abs() < 0.001
        );
        assert!(scrap.variant < 4);
        assert_eq!(
            world.get::<Collider>(e).unwrap().shape,
            tungsten::physics::Shape::Aabb {
                half_extents: SCRAP_HALF
            }
        );
        assert!(world.get::<Velocity>(e).unwrap().0.is_finite());
        assert!(world.get::<Velocity>(e).unwrap().0.length() > 100.0);
        assert!((p.0 - at).abs().cmple(BRICK_HALF - SCRAP_HALF).all());
        assert!(!world.has::<Frozen>(e));
    }
}

#[test]
fn thermal_contact_uses_surface_distance_and_leaves_warm_and_distant_iron_intact() {
    let mut world = seed_world();
    let radius = FIREBALL_BLAST_RADIUS;
    let edge = spawn_brick(&mut world, Vec2::new(BRICK_HALF.x + radius, 0.0));
    let distant = spawn_brick(&mut world, Vec2::new(-(BRICK_HALF.x + radius + 1.0), 0.0));
    let warm = spawn_brick(&mut world, Vec2::ZERO);
    freeze_iron(&mut world, edge);
    freeze_iron(&mut world, distant);
    explode_fireball(&mut world, Vec2::ZERO);
    assert!(!world.is_alive(edge));
    assert!(world.is_alive(distant));
    assert!(world.is_alive(warm));
    assert_eq!(world.query::<(Entity, &IronScrap)>().count(), SCRAP_COUNT);
    explode_fireball(&mut world, Vec2::ZERO);
    assert_eq!(
        world.query::<(Entity, &IronScrap)>().count(),
        SCRAP_COUNT,
        "warm scraps do not split again"
    );
}

#[test]
fn frozen_iron_slides_with_reduced_friction_and_scraps_can_refreeze() {
    let mut world = seed_world();
    let warm = spawn_brick(&mut world, Vec2::ZERO);
    let cold = spawn_brick(&mut world, Vec2::new(400.0, 0.0));
    freeze_iron(&mut world, cold);
    for (i, e) in [warm, cold].into_iter().enumerate() {
        world.get_mut::<Velocity>(e).unwrap().0 = Vec2::new(-100.0, 0.0);
        world
            .get_resource_mut::<EventQueue<CollisionEvent>>()
            .unwrap()
            .send(CollisionEvent {
                a: e,
                b: None,
                normal: -Vec2::Y,
                penetration: 0.0,
            });
        if i == 1 {
            freeze_iron(&mut world, e);
        }
    }
    let dt = world.get_resource::<Time>().unwrap().delta();
    brick_friction(&mut world);
    assert!((world.get::<Velocity>(warm).unwrap().0.x + 100.0 - BRICK_FRICTION * dt).abs() < 0.001);
    assert!(
        (world.get::<Velocity>(cold).unwrap().0.x + 100.0 - FROZEN_FRICTION * dt).abs() < 0.001
    );
    explode_fireball(&mut world, Vec2::new(400.0, 0.0));
    let scrap = world.query::<(Entity, &IronScrap)>().next().unwrap().0;
    freeze_iron(&mut world, scrap);
    assert!(world.has::<Frozen>(scrap));
    let at = world.get::<Position>(scrap).unwrap().0;
    explode_fireball(&mut world, at);
    assert!(!world.is_alive(scrap));
    assert_eq!(
        world.query::<(Entity, &IronScrap)>().count(),
        SCRAP_COUNT - 1
    );
}

#[test]
fn scrap_friction_is_one_sixteenth_of_full_iron_with_the_same_frozen_reduction() {
    let mut world = seed_world();
    let brick = spawn_brick(&mut world, Vec2::ZERO);
    freeze_iron(&mut world, brick);
    crate::brick::thermal_shatter(&mut world, Vec2::ZERO, FIREBALL_BLAST_RADIUS);
    let scraps: Vec<_> = world
        .query::<(Entity, &IronScrap)>()
        .map(|(e, _)| e)
        .take(2)
        .collect();
    let warm = scraps[0];
    let cold = scraps[1];
    freeze_iron(&mut world, cold);
    for (index, scrap) in scraps.into_iter().enumerate() {
        world.get_mut::<Velocity>(scrap).unwrap().0 = Vec2::new(100.0, 0.0);
        // Support contacts may put the scrap on either side of the pair.
        world
            .get_resource_mut::<EventQueue<CollisionEvent>>()
            .unwrap()
            .send(CollisionEvent {
                a: if index == 0 { scrap } else { warm },
                b: (index == 1).then_some(scrap),
                normal: if index == 0 { -Vec2::Y } else { Vec2::Y },
                penetration: 0.0,
            });
    }
    let dt = world.get_resource::<Time>().unwrap().delta();
    brick_friction(&mut world);
    assert!(
        (world.get::<Velocity>(warm).unwrap().0.x - (100.0 - BRICK_FRICTION / 16.0 * dt)).abs()
            < 0.001
    );
    assert!(
        (world.get::<Velocity>(cold).unwrap().0.x - (100.0 - FROZEN_FRICTION / 16.0 * dt)).abs()
            < 0.001
    );
}

#[test]
fn repeated_channels_and_long_holds_keep_particles_bounded_and_stop_on_death() {
    let mut harness = ice_harness();
    load_presentation_assets(harness.world_mut());
    spawn_test_player(harness.world_mut(), Vec2::new(100.0, 100.0));
    for _ in 0..20 {
        aim_and_press(harness.world_mut(), Vec2::new(1500.0, 100.0));
        harness.step(12);
        assert_eq!(
            harness
                .world()
                .query::<(Entity, &ParticleEmitter)>()
                .count(),
            3
        );
        input_mut(&mut harness).key_up(KeyCode::KeyF);
        harness.step(1);
        assert_eq!(
            harness
                .world()
                .query::<(Entity, &ParticleEmitter)>()
                .count(),
            0
        );
        assert!(harness.world().query::<(Entity, &Particle)>().count() <= 2048);
    }
    aim_and_press(harness.world_mut(), Vec2::new(1500.0, 100.0));
    harness.step(600);
    let world = harness.world_mut();
    assert_eq!(world.query::<(Entity, &IceBeam)>().count(), 1);
    assert_eq!(world.query::<(Entity, &ParticleEmitter)>().count(), 3);
    assert!(world.query::<(Entity, &Particle)>().count() <= 292);
    assert_eq!(
        world.get_resource::<ParticleBudget>().unwrap().global_cap,
        2048
    );
    assert_eq!(world.query::<(Entity, &TransientEmitter)>().count(), 0);
    world.insert_resource(crate::death::DeathScreen::Dying { elapsed: 0.0 });
    cast_ice_beam_system(world);
    assert_eq!(world.query::<(Entity, &IceBeam)>().count(), 0);
    assert_eq!(world.query::<(Entity, &ParticleEmitter)>().count(), 0);
    harness.step(120);
    assert_eq!(harness.world().query::<(Entity, &Particle)>().count(), 0);
}

#[test]
fn spray_aim_tracks_the_caster_and_partial_frost_thaws_when_aim_moves_away() {
    let mut world = seed_world();
    load_presentation_assets(&mut world);
    let player = spawn_test_player(&mut world, Vec2::new(100.0, 100.0));
    let right = spawn_brick(&mut world, Vec2::new(350.0, 100.0));
    let below = spawn_brick(&mut world, Vec2::new(100.0, 350.0));
    aim_and_press(&mut world, Vec2::new(350.0, 100.0));
    spray_steps(&mut world, 45);
    assert!((freeze_amount(&world, right) - 0.5).abs() < 0.001);
    assert_eq!(freeze_amount(&world, below), 0.0);
    aim_and_press(&mut world, Vec2::new(100.0, 350.0));
    world.get_mut::<Position>(player).unwrap().0.x += 10.0;
    ice_beam_presentation(&mut world);
    let beam = world.query::<(Entity, &IceBeam)>().next().unwrap().1;
    assert!(beam.end.y > beam.start.y + 150.0);
    assert!((beam.start.x - 109.04).abs() < 0.1);
    spray_steps(&mut world, 90);
    assert!(world.has::<Frozen>(below));
    assert!(!world.has::<Frozen>(right));
    assert!((freeze_amount(&world, right) - 0.275).abs() < 0.001);
    world
        .get_resource_mut::<InputState>()
        .unwrap()
        .key_up(KeyCode::KeyF);
    spray_steps(&mut world, 120);
    assert!(!world.has::<Chill>(right));
    assert!(world.has::<Frozen>(below), "completed freezing persists");
}

#[test]
fn release_death_and_owner_removal_stop_the_loop_even_without_a_fixed_step() {
    for stop in 0..3 {
        let mut world = seed_world();
        load_presentation_assets(&mut world);
        world.insert_resource(ice_sounds());
        world.insert_resource(AudioCommands::new());
        let player = spawn_test_player(&mut world, Vec2::new(100.0, 100.0));
        aim_and_press(&mut world, Vec2::new(400.0, 100.0));
        cast_ice_beam_system(&mut world);
        world.get_resource_mut::<AudioCommands>().unwrap().drain();
        let time = world.get_resource_mut::<Time>().unwrap();
        time.pause();
        time.advance_frame(1.0 / 144.0);
        match stop {
            0 => world
                .get_resource_mut::<InputState>()
                .unwrap()
                .key_up(KeyCode::KeyF),
            1 => world.insert_resource(crate::death::DeathScreen::Dying { elapsed: 0.0 }),
            _ => {
                world.despawn(player);
            }
        }
        ice_beam_cleanup(&mut world);
        ice_beam_cleanup(&mut world);
        assert_eq!(world.query::<(Entity, &IceBeam)>().count(), 0);
        assert_eq!(world.query::<(Entity, &ParticleEmitter)>().count(), 0);
        let audio = world.get_resource_mut::<AudioCommands>().unwrap().drain();
        assert!(matches!(
            audio.first(),
            Some(AudioCommand::Stop {
                handle: AudioHandle(7)
            })
        ));
        if stop == 0 {
            assert!(matches!(
                audio.as_slice(),
                [
                    AudioCommand::Stop { .. },
                    AudioCommand::Play {
                        handle: AudioHandle(10),
                        looping: false,
                        ..
                    }
                ]
            ));
        } else {
            assert_eq!(audio.len(), 1, "death and owner loss should stop quietly");
        }
    }
}

#[test]
fn freezing_uses_fixed_exposure_at_30_60_and_144_hz_and_stops_when_paused() {
    for fps in [30, 60, 144] {
        let mut harness = ice_harness();
        let world = harness.world_mut();
        spawn_test_player(world, Vec2::new(100.0, 100.0));
        let iron = spawn_brick(world, Vec2::new(350.0, 100.0));
        aim_and_press(world, Vec2::new(350.0, 100.0));
        harness.set_dt(1.0 / fps as f32);
        harness.step(fps);
        let amount = freeze_amount(harness.world(), iron);
        assert!(
            (amount - 1.0 / FREEZE_TIME).abs() < 0.012,
            "{fps} Hz: {amount}"
        );
        assert!(!harness.world().has::<Frozen>(iron));
        harness
            .world_mut()
            .get_resource_mut::<Time>()
            .unwrap()
            .pause();
        harness.step(fps);
        assert_eq!(freeze_amount(harness.world(), iron), amount);
        harness
            .world_mut()
            .get_resource_mut::<Time>()
            .unwrap()
            .resume();
        harness.step(fps / 2 + 1);
        assert!(
            harness.world().has::<Frozen>(iron),
            "{fps} Hz did not complete"
        );
    }
}

#[test]
fn cone_frosts_iron_beside_the_centerline_and_spray_particles_follow_aim() {
    let mut harness = ice_harness();
    let world = harness.world_mut();
    load_presentation_assets(world);
    spawn_test_player(world, Vec2::new(100.0, 100.0));
    let offset = spawn_brick(world, Vec2::new(380.0, 180.0));
    aim_and_press(world, Vec2::new(450.0, 100.0));
    harness.step(90);
    assert!(
        harness.world().has::<Frozen>(offset),
        "the side of the cone should frost iron"
    );
    let world = harness.world();
    let config = world
        .get_resource::<ParticleConfigRegistry>()
        .unwrap()
        .id_for_name("ex10_ice_contact")
        .unwrap();
    let (contact, _) = world
        .query::<(Entity, &ParticleEmitter)>()
        .find(|(_, emitter)| emitter.config == config)
        .unwrap();
    assert!(
        !world
            .get::<tungsten::core::ParticleEmitterState>(contact)
            .unwrap()
            .drained
    );
    let at = world.get::<Transform>(contact).unwrap().position;
    assert!(
        at.y > 110.0 && at.x < 380.0,
        "side contact particles should appear on the contacted iron face: {at}"
    );
    // Aim upward into empty space and allow the old rightward particles to drain.
    aim_and_press(harness.world_mut(), Vec2::new(100.0, -400.0));
    harness.step(120);
    let world = harness.world();
    let spray = world
        .get_resource::<ParticleConfigRegistry>()
        .unwrap()
        .id_for_name("ex10_ice_beam")
        .unwrap();
    let velocities: Vec<_> = world
        .query::<(Entity, &Particle)>()
        .filter(|(_, p)| {
            p.age > 0.0
                && p.emitter.is_some_and(|e| {
                    world
                        .get::<ParticleEmitter>(e)
                        .is_some_and(|emitter| emitter.config == spray)
                })
        })
        .map(|(_, p)| p.velocity)
        .collect();
    assert!(!velocities.is_empty());
    assert!(
        velocities
            .iter()
            .all(|v| v.y < -500.0 && v.x.abs() < -v.y * 0.26),
        "snow crystals should travel inside the upward cone"
    );
}

#[test]
fn high_refresh_press_starts_the_spray_before_the_next_physics_step() {
    let mut harness = ice_harness();
    let world = harness.world_mut();
    load_presentation_assets(world);
    world.insert_resource(ice_sounds());
    spawn_test_player(world, Vec2::new(100.0, 100.0));
    let iron = spawn_brick(world, Vec2::new(350.0, 100.0));
    aim_and_press(world, Vec2::new(350.0, 100.0));
    harness.set_dt(1.0 / 144.0);
    harness.step(1);
    let world = harness.world();
    assert_eq!(
        world
            .get_resource::<Time>()
            .unwrap()
            .fixed_steps_this_frame(),
        0
    );
    assert_eq!(world.query::<(Entity, &IceBeam)>().count(), 1);
    assert!(world.query::<(Entity, &Particle)>().count() > 0);
    assert_eq!(
        freeze_amount(world, iron),
        0.0,
        "exposure still waits for simulation"
    );
    assert_eq!(harness.audio().len(), 2, "one startup sound and one loop");
    input_mut(&mut harness).key_up(KeyCode::KeyF);
    harness.step(1);
    assert_eq!(harness.world().query::<(Entity, &IceBeam)>().count(), 0);
    assert_eq!(freeze_amount(harness.world(), iron), 0.0);
}

#[test]
fn simultaneous_freezes_and_thermal_fractures_share_audio_but_keep_individual_cues() {
    let mut harness = ice_harness();
    let world = harness.world_mut();
    load_presentation_assets(world);
    world.insert_resource(ice_sounds());
    spawn_test_player(world, Vec2::new(100.0, 100.0));
    let upper = spawn_brick(world, Vec2::new(350.0, 30.0));
    let lower = spawn_brick(world, Vec2::new(350.0, 170.0));
    aim_and_press(world, Vec2::new(450.0, 100.0));
    harness.step(90);
    assert!(harness.world().has::<Frozen>(upper));
    assert!(harness.world().has::<Frozen>(lower));
    assert!(matches!(
        harness.audio(),
        [AudioCommand::Play {
            handle: AudioHandle(8),
            looping: false,
            ..
        }]
    ));
    let world = harness.world_mut();
    assert_eq!(
        world
            .query::<(Entity, &IcePulse)>()
            .filter(|(_, p)| !p.shatter)
            .count(),
        2
    );
    crate::brick::thermal_shatter(world, Vec2::new(350.0, 100.0), FIREBALL_BLAST_RADIUS);
    assert_eq!(world.query::<(Entity, &IronScrap)>().count(), 32);
    assert!(matches!(
        world
            .get_resource_mut::<AudioCommands>()
            .unwrap()
            .drain()
            .as_slice(),
        [AudioCommand::Play {
            handle: AudioHandle(11),
            looping: false,
            ..
        }]
    ));
    assert_eq!(
        world
            .query::<(Entity, &IcePulse)>()
            .filter(|(_, p)| p.shatter)
            .count(),
        2
    );
    input_mut(&mut harness).key_up(KeyCode::KeyF);
    harness.step(120);
    assert_eq!(harness.world().query::<(Entity, &IcePulse)>().count(), 0);
}

#[test]
fn cold_pulses_draw_without_scene_lighting_stay_bounded_and_expire() {
    let mut harness = ice_harness();
    let world = harness.world_mut();
    world.insert_resource(ice_sounds());
    let assets = world
        .get_resource_mut::<tungsten::core::AssetRegistry>()
        .unwrap();
    mock_sprite(assets, "ex10_ice_ring", 71, false);
    mock_sprite(assets, "ex10_flame_glow", 72, false);
    for _ in 0..64 {
        crate::ice::ice_pulse(world, Vec2::new(200.0, 100.0), 128.0, true);
    }
    assert_eq!(
        world.query::<(Entity, &IcePulse)>().count(),
        crate::state::TRANSIENT_EMITTER_CAP
    );
    let cues = crate::extract::extract_sprites(world);
    assert!(cues.iter().all(|batch| !batch.lit));
    assert_eq!(
        cues.iter()
            .map(|batch| batch.instances.len())
            .sum::<usize>(),
        32
    );
    assert!(matches!(
        world
            .get_resource_mut::<AudioCommands>()
            .unwrap()
            .drain()
            .as_slice(),
        [AudioCommand::Play {
            handle: AudioHandle(11),
            ..
        }]
    ));
    harness.step(30);
    assert_eq!(harness.world().query::<(Entity, &IcePulse)>().count(), 0);
}
