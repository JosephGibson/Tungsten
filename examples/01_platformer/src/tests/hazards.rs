use super::*;
use tungsten::core::With;

#[test]
fn audio_controls_and_damage_flash_shake_remain_wired() {
    use tungsten::core::{MaterialAssetId, ShakeEvent, Tween, UniformOverrideBlock};
    let mut harness =
        platformer_harness(&[("audio_input_system", crate::systems::audio_input_system)]);
    let world = harness.world_mut();
    world.insert_resource(AudioCommands::new());
    world.insert_resource(AudioState {
        sfx_handle: AudioHandle(1),
        black_hole_sfx_handle: AudioHandle(2),
        music_handle: AudioHandle(3),
        sfx_volume: 1.0,
        black_hole_sfx_volume: 0.65,
        music_volume: 0.4,
        music_playing: false,
        master_volume: 0.5,
    });
    input_mut(&mut harness).key_down(KeyCode::KeyM);
    harness.step(1);
    assert!(
        harness
            .world()
            .get_resource::<AudioState>()
            .unwrap()
            .music_playing
    );
    assert!(matches!(
        harness.audio()[0],
        AudioCommand::Play { looping: true, .. }
    ));
    let input = input_mut(&mut harness);
    input.key_up(KeyCode::KeyM);
    input.key_down(KeyCode::KeyS);
    harness.step(1);
    let world = harness.world_mut();
    assert!(!world.get_resource::<AudioState>().unwrap().music_playing);
    world.insert_resource(EventQueue::<ShakeEvent>::new());
    let player = spawn_test_player(world, PLAYER_SPAWN);
    world.insert(
        player,
        crate::state::PlayerMaterial {
            material_id: MaterialAssetId(0),
        },
    );
    world.insert(player, UniformOverrideBlock::default());
    let ball = world.spawn();
    world.insert(ball, Ball);
    world
        .get_resource_mut::<EventQueue<CollisionEvent>>()
        .unwrap()
        .send(CollisionEvent {
            a: player,
            b: Some(ball),
            normal: Vec2::X,
            penetration: 1.0,
        });
    world.insert(player, crate::gameplay::Health::default());
    crate::gameplay::hazard_contacts(world);
    assert_eq!(
        world.get::<crate::gameplay::Health>(player).unwrap().hearts,
        3
    );
    assert!(world.get::<Tween>(player).is_none());
    assert!(
        world
            .get_resource::<EventQueue<ShakeEvent>>()
            .unwrap()
            .is_empty()
    );
    crate::gameplay::damage_player(world, player, PLAYER_SPAWN + Vec2::X);
    assert!(world.get::<Tween>(player).is_some());
    assert!(
        !world
            .get_resource::<EventQueue<ShakeEvent>>()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn hazards_damage_once_per_cooldown_and_restore_health_on_respawn() {
    use crate::gameplay::*;
    let mut world = seed_world();
    let player = spawn_test_player(&mut world, Vec2::new(100.0, 100.0));
    let spike = world.spawn();
    world.insert(spike, Hazard { fire: false });
    world.insert(spike, Position(Vec2::new(100.0, 100.0)));
    hazard_contacts(&mut world);
    assert_eq!(world.get::<Health>(player).unwrap().hearts, 2);
    assert!(world.get::<Velocity>(player).unwrap().0.y < 0.0);
    for _ in 0..10 {
        hazard_contacts(&mut world);
    }
    assert_eq!(world.get::<Health>(player).unwrap().hearts, 2);
    world.get_mut::<Health>(player).unwrap().immunity = 0.0;
    hazard_contacts(&mut world);
    assert_eq!(world.get::<Health>(player).unwrap().hearts, 1);
    world.get_mut::<Health>(player).unwrap().immunity = 0.0;
    hazard_contacts(&mut world);
    assert_eq!(world.get::<Health>(player).unwrap().hearts, 3);
    assert_eq!(world.get::<Position>(player).unwrap().0, PLAYER_SPAWN);
    assert_eq!(world.get::<Velocity>(player).unwrap().0, Vec2::ZERO);
    assert!(world.get::<Health>(player).unwrap().immunity > 1.0);
}

#[test]
fn moving_fire_sweeps_fast_balls_once_and_explosions_do_not_hurt_players() {
    use crate::gameplay::*;
    let mut harness = platformer_harness(&[
        ("move_obstacles", move_obstacles),
        (
            "transient_emitter_cleanup",
            crate::systems::transient_emitter_cleanup,
        ),
    ]);
    let world = harness.world_mut();
    load_presentation_assets(world);
    let player = spawn_test_player(world, Vec2::new(100.0, 200.0));
    world.insert(player, Health::default());
    for _ in 0..2 {
        let fire = world.spawn();
        world.insert(fire, Hazard { fire: true });
        world.insert(fire, Position(Vec2::new(100.0, 100.0)));
        world.insert(fire, PreviousPosition(Vec2::new(100.0, 300.0)));
    }
    // Both the ball and moving fire cross between frames.
    let ball = world.spawn();
    world.insert(ball, Ball);
    world.insert(ball, PreviousPosition(Vec2::new(0.0, 200.0)));
    world.insert(ball, Position(Vec2::new(200.0, 200.0)));
    hazard_contacts(world);
    assert!(world.get::<Ball>(ball).is_none());
    assert_eq!(world.query::<(Entity, &Explosion)>().count(), 1);
    assert_eq!(
        world
            .query::<(Entity, &crate::state::TransientEmitter)>()
            .count(),
        1
    );
    // The crossing fire hit the player once. Neither overlapping fire nor the
    // ball burst can add another hit during immunity.
    assert_eq!(world.get::<Health>(player).unwrap().hearts, 2);
    for e in world
        .query_filtered::<Entity, With<Hazard>>()
        .collect::<Vec<_>>()
    {
        world.despawn(e);
    }
    world.get_mut::<Health>(player).unwrap().immunity = 0.0;
    hazard_contacts(world);
    assert_eq!(world.get::<Health>(player).unwrap().hearts, 2);
    harness.step(150);
    let world = harness.world();
    assert_eq!(world.query::<(Entity, &Explosion)>().count(), 0);
    assert_eq!(
        world
            .query::<(Entity, &crate::state::TransientEmitter)>()
            .count(),
        0
    );
}

#[test]
fn explosion_bursts_and_rings_stay_bounded_under_dense_ball_contact() {
    use crate::gameplay::*;
    let mut world = seed_world();
    load_presentation_assets(&mut world);
    let fire = world.spawn();
    world.insert(fire, Hazard { fire: true });
    world.insert(fire, Position(Vec2::ZERO));
    for _ in 0..100 {
        let ball = world.spawn();
        world.insert(ball, Ball);
        world.insert(ball, Position(Vec2::ZERO));
    }
    hazard_contacts(&mut world);
    assert_eq!(world.query::<(Entity, &Ball)>().count(), 0);
    assert_eq!(
        world.query::<(Entity, &Explosion)>().count(),
        crate::state::TRANSIENT_EMITTER_CAP
    );
    assert_eq!(
        world
            .query::<(Entity, &crate::state::TransientEmitter)>()
            .count(),
        crate::state::TRANSIENT_EMITTER_CAP
    );
}

#[test]
fn moving_fire_emits_a_bounded_particle_trail() {
    use crate::gameplay::*;
    use tungsten::core::{Particle, ParticleEmitter, ParticleEmitterState};
    let mut harness = platformer_harness(&[
        ("move_obstacles", move_obstacles),
        ("sync_position_to_transform", sync_position_to_transform),
        (
            "transient_emitter_cleanup",
            crate::systems::transient_emitter_cleanup,
        ),
    ]);
    let world = harness.world_mut();
    load_presentation_assets(world);
    spawn_obstacles(world);
    let flames: Vec<_> = world
        .query::<(Entity, &Hazard)>()
        .filter(|(_, h)| h.fire)
        .map(|(e, _)| e)
        .collect();
    assert_eq!(flames.len(), 6);
    assert!(
        flames
            .iter()
            .all(|e| world.get::<ParticleEmitter>(*e).is_some())
    );
    harness.step(120);
    let world = harness.world();
    assert!(world.query::<(Entity, &Particle)>().count() > 60);
    assert!(world.query::<(Entity, &Particle)>().count() <= 240);
    for e in flames {
        assert!(world.get::<ParticleEmitterState>(e).unwrap().active_count <= 40);
    }
}

#[test]
fn fireballs_face_their_travel_and_drag_anchored_drips() {
    use crate::gameplay::{
        EmitterAnchor, FIREBALL_DRIP_OFFSET, Hazard, fireball_faces_left, motion_velocity,
        move_obstacles, spawn_obstacles,
    };
    use tungsten::core::{ParticleConfig, ParticleConfigRegistry, ParticleEmitter};
    let mut world = seed_world();
    load_presentation_assets(&mut world);
    let path = asset_path("particles/fireball_drips.json");
    let config = ParticleConfig::load(&path).unwrap();
    world
        .get_resource_mut::<ParticleConfigRegistry>()
        .unwrap()
        .register("ex10_fireball_drips".into(), path, config);
    spawn_test_player(&mut world, PLAYER_SPAWN);
    spawn_obstacles(&mut world);
    let fires: Vec<_> = world
        .query::<(Entity, &Hazard)>()
        .filter(|(_, h)| h.fire)
        .map(|(e, _)| e)
        .collect();
    let drips: Vec<_> = world
        .query::<(Entity, &EmitterAnchor)>()
        .map(|(e, a)| (e, a.parent))
        .collect();
    assert_eq!(drips.len(), fires.len());
    assert!(
        drips
            .iter()
            .all(|(e, parent)| fires.contains(parent) && world.get::<ParticleEmitter>(*e).is_some())
    );
    let mut seen = vec![[false; 2]; fires.len()];
    for _ in 0..450 {
        move_obstacles(&mut world);
        for (i, fire) in fires.iter().enumerate() {
            let velocity = motion_velocity(&world, *fire).unwrap();
            let left = fireball_faces_left(&world, *fire);
            if velocity.x == 0.0 {
                // Vertical movers watch the player, who waits to their left.
                assert!(left);
            } else {
                assert_eq!(left, velocity.x < 0.0);
            }
            seen[i][usize::from(left)] = true;
        }
        for (drip, parent) in &drips {
            let expected = world.get::<Position>(*parent).unwrap().0 + FIREBALL_DRIP_OFFSET;
            assert_eq!(world.get::<Transform>(*drip).unwrap().position, expected);
        }
    }
    // Over a full period every horizontal mover turns both ways.
    for (i, fire) in fires.iter().enumerate() {
        if motion_velocity(&world, *fire).unwrap().x != 0.0 {
            assert_eq!(seen[i], [true, true]);
        }
    }
}
