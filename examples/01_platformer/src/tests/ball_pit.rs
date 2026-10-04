use super::*;
use crate::gameplay::{move_obstacles, small_ball_impacts};
use crate::state::{
    BALL_ANIMATION_ID, BALL_CAP, BALL_RESTITUTION, BALL_START_SPRITE_ID, BALL_VISUAL_DIAMETER,
    SMALL_BALL_ANIMATION_ID, SMALL_BALL_BURSTS_PER_FRAME, SMALL_BALL_IMPACT_SPEED,
    SMALL_BALL_SCALE, SMALL_BALL_START_SPRITE_ID, SmallBall, TRANSIENT_EMITTER_CAP,
    TransientEmitter,
};
use tungsten::core::{AssetRegistry, Particle, ParticleEmitter};
use tungsten::physics::Shape;

#[test]
fn middle_mouse_has_five_times_the_rate_half_size_and_dedicated_animation() {
    for (dt, frames, expected_normal, expected_small) in [
        (0.001, 322, 10, 50),
        (1.0 / 60.0, 20, 10, 52),
        (1.0 / 30.0, 10, 10, 52),
        (0.161, 1, 5, 25),
    ] {
        let mut harness = platformer_harness(&[("spawn_ball_system", spawn_ball_system)]);
        let world = harness.world_mut();
        load_presentation_assets(world);
        crate::setup::platformer_bindings(world);
        world.insert_resource(BallSpawnState::default());
        world
            .get_resource_mut::<ActionMap>()
            .unwrap()
            .replace_bindings(
                "spawn_ball",
                vec![Binding::Mouse {
                    button: MouseButton::Left,
                }],
            );
        harness.set_dt(dt);
        let input = input_mut(&mut harness);
        input.update_cursor_position(240.0, 144.0);
        input.mouse_down(MouseButton::Left);
        input.mouse_down(MouseButton::Middle);
        // Independent timers preserve the rate at high and low frame rates.
        harness.step(frames);
        let world = harness.world_mut();
        let small = world.query::<SmallBall>().count();
        let normal = world.query::<Ball>().count() - small;
        assert_eq!(normal, expected_normal);
        assert_eq!(small, expected_small, "dt={dt}");
        let mut assets = AssetRegistry::new();
        mock_sprite(&mut assets, BALL_START_SPRITE_ID, 901, false);
        mock_sprite(&mut assets, SMALL_BALL_START_SPRITE_ID, 902, false);
        world.insert_resource(assets);
        for (entity, _) in world.query::<Ball>() {
            let small = world.get::<SmallBall>(entity).is_some();
            let scale = if small { SMALL_BALL_SCALE } else { 1.0 };
            assert_eq!(
                world.get::<Collider>(entity).unwrap().shape,
                Shape::Circle {
                    radius: BALL_RADIUS * scale
                }
            );
            assert_eq!(
                world.get::<RigidBody>(entity).unwrap().restitution,
                BALL_RESTITUTION
            );
            assert_eq!(
                world.get::<CurrentSprite>(entity).unwrap().0,
                if small {
                    SMALL_BALL_START_SPRITE_ID
                } else {
                    BALL_START_SPRITE_ID
                }
            );
            let registry = world
                .get_resource::<tungsten::core::AnimationRegistry>()
                .unwrap();
            let expected = AnimationState::new(if small {
                SMALL_BALL_ANIMATION_ID
            } else {
                BALL_ANIMATION_ID
            });
            assert_eq!(
                world
                    .get::<AnimationState>(entity)
                    .unwrap()
                    .current_sprite(registry),
                expected.current_sprite(registry)
            );
        }
        let batches = crate::extract::extract_sprites(world);
        for (texture, count) in [(901, normal), (902, small)] {
            assert_eq!(
                batches
                    .iter()
                    .filter(|b| b.texture.0 == texture)
                    .map(|b| b.instances.len())
                    .sum::<usize>(),
                count
            );
        }
        for batch in batches {
            if matches!(batch.texture.0, 901 | 902) {
                let diameter =
                    BALL_VISUAL_DIAMETER * if batch.texture.0 == 902 { 0.5 } else { 1.0 };
                assert!(batch.instances.iter().all(|i| i.size == [diameter; 2]));
            }
        }
        world
            .get_resource_mut::<InputState>()
            .unwrap()
            .mouse_up(MouseButton::Middle);
        spawn_ball_system(world);
        assert_eq!(
            world
                .get_resource::<BallSpawnState>()
                .unwrap()
                .small_accumulator,
            0.0
        );
        assert_eq!(world.query::<SmallBall>().count(), small);
    }
}

#[test]
fn spawning_stops_at_the_ball_cap() {
    let mut harness = platformer_harness(&[("spawn_ball_system", spawn_ball_system)]);
    let world = harness.world_mut();
    crate::setup::platformer_bindings(world);
    world.insert_resource(BallSpawnState::default());
    world
        .get_resource_mut::<ActionMap>()
        .unwrap()
        .replace_bindings(
            "spawn_ball",
            vec![Binding::Mouse {
                button: MouseButton::Left,
            }],
        );
    for _ in 0..BALL_CAP - 1 {
        let ball = world.spawn();
        world.insert(ball, Ball);
    }
    // One frame long enough for 5 large and 25 small balls.
    harness.set_dt(0.161);
    let input = input_mut(&mut harness);
    input.update_cursor_position(240.0, 144.0);
    input.mouse_down(MouseButton::Left);
    input.mouse_down(MouseButton::Middle);

    // The one ball left in the budget goes out, then nothing.
    for _ in 0..2 {
        harness.step(1);
        assert_eq!(harness.world().query::<Ball>().count(), BALL_CAP);
    }
}

#[test]
fn audio_stop_is_s_only_even_after_shared_input_reload() {
    let mut app = App::new(Config::default()).unwrap();
    crate::setup::configure_app(&mut app);
    let world = app.world_mut();
    for reload in [false, true] {
        if reload {
            world.insert_resource(ActionMap::default_map());
            crate::setup::platformer_bindings(world);
        }
        world.insert_resource(AudioState {
            sfx_handle: AudioHandle(0),
            black_hole_sfx_handle: AudioHandle(1),
            music_handle: AudioHandle(2),
            sfx_volume: 1.0,
            black_hole_sfx_volume: 1.0,
            music_volume: 1.0,
            music_playing: true,
            master_volume: 0.5,
        });
        world.insert_resource(AudioCommands::new());
        world.insert_resource(InputState::new());
        world
            .get_resource_mut::<InputState>()
            .unwrap()
            .mouse_down(MouseButton::Middle);
        crate::systems::audio_input_system(world);
        assert!(world.get_resource::<AudioState>().unwrap().music_playing);
        world
            .get_resource_mut::<InputState>()
            .unwrap()
            .key_down(KeyCode::KeyS);
        crate::systems::audio_input_system(world);
        assert!(!world.get_resource::<AudioState>().unwrap().music_playing);
        assert_eq!(
            world
                .get_resource::<ActionMap>()
                .unwrap()
                .bindings("audio_stop_all"),
            [Binding::Key {
                code: KeyCode::KeyS
            }]
        );
    }
    let hud = crate::extract::extract_text(world);
    assert!(
        hud.iter()
            .any(|t| t.content.contains("MMB small balls (5x)"))
    );
    assert!(hud.iter().any(|t| t.content.contains("S stop")));
    assert!(hud.iter().all(|t| !t.content.contains("S/MMB")));
}

fn impact_ball(world: &mut World, velocity: Vec2, small: bool) -> tungsten::core::Entity {
    let e = world.spawn();
    world.insert(e, Ball);
    if small {
        world.insert(e, SmallBall::default());
    }
    world.insert(e, Position(Vec2::new(200.0, 200.0)));
    world.insert(e, Velocity(velocity));
    e
}

fn floor_contact(world: &mut World, e: tungsten::core::Entity) {
    world
        .get_resource_mut::<EventQueue<CollisionEvent>>()
        .unwrap()
        .send(CollisionEvent {
            a: e,
            b: None,
            normal: -Vec2::Y,
            penetration: 0.1,
        });
}

#[test]
fn impacts_require_small_ball_and_closing_speed_strictly_above_threshold() {
    for (velocity, small, expected) in [
        (Vec2::Y * (SMALL_BALL_IMPACT_SPEED + 1.0), true, 1),
        (Vec2::Y * SMALL_BALL_IMPACT_SPEED, true, 0),
        (Vec2::Y * (SMALL_BALL_IMPACT_SPEED - 1.0), true, 0),
        (Vec2::new(2000.0, 20.0), true, 0),
        (-Vec2::Y * 1000.0, true, 0),
        (Vec2::Y * 1000.0, false, 0),
    ] {
        let mut world = seed_world();
        load_presentation_assets(&mut world);
        let e = impact_ball(&mut world, velocity, small);
        move_obstacles(&mut world);
        // Resolution stopped the ball: the trigger must use the captured speed.
        world.get_mut::<Velocity>(e).unwrap().0 = Vec2::ZERO;
        floor_contact(&mut world, e);
        small_ball_impacts(&mut world);
        assert_eq!(world.query::<TransientEmitter>().count(), expected);
        small_ball_impacts(&mut world);
        assert_eq!(
            world.query::<TransientEmitter>().count(),
            expected,
            "per-ball cooldown"
        );
    }
}

#[test]
fn relative_body_impacts_work_on_b_side_and_dense_contacts_remain_bounded() {
    let mut harness = platformer_harness(&[(
        "transient_emitter_cleanup",
        crate::systems::transient_emitter_cleanup,
    )]);
    let world = harness.world_mut();
    load_presentation_assets(world);
    let a = impact_ball(world, Vec2::X * 300.0, false);
    let b = impact_ball(world, -Vec2::X * 300.0, true);
    move_obstacles(world);
    world
        .get_resource_mut::<EventQueue<CollisionEvent>>()
        .unwrap()
        .send(CollisionEvent {
            a,
            b: Some(b),
            normal: -Vec2::X,
            penetration: 0.1,
        });
    small_ball_impacts(world);
    assert_eq!(world.query::<TransientEmitter>().count(), 1);
    for e in world.query_entities::<TransientEmitter>() {
        world.despawn(e);
    }
    for _ in 0..200 {
        let e = impact_ball(world, Vec2::Y * 1000.0, true);
        // Duplicate substep contacts must not consume extra burst slots.
        for _ in 0..8 {
            floor_contact(world, e);
        }
    }
    move_obstacles(world);
    small_ball_impacts(world);
    assert_eq!(
        world.query::<TransientEmitter>().count(),
        SMALL_BALL_BURSTS_PER_FRAME
    );
    for _ in 0..20 {
        small_ball_impacts(world);
    }
    assert_eq!(
        world.query::<TransientEmitter>().count(),
        TRANSIENT_EMITTER_CAP
    );
    harness.step(50);
    let world = harness.world_mut();
    crate::systems::transient_emitter_cleanup(world);
    assert_eq!(world.query::<TransientEmitter>().count(), 0);
}

#[test]
fn impact_particles_span_rainbow_and_vortex_births_spiral_inward() {
    let mut harness = platformer_harness(&[(
        "transient_emitter_cleanup",
        crate::systems::transient_emitter_cleanup,
    )]);
    let world = harness.world_mut();
    load_presentation_assets(world);
    crate::systems::spawn_transient_effect(world, "ex10_small_ball_impact", Vec2::ZERO);
    harness.step(1);
    let world = harness.world_mut();
    crate::gameplay::scene_effects(world);
    let colors: Vec<_> = world
        .query::<Particle>()
        .map(|(_, p)| p.base_rgba)
        .collect();
    for channel in 0..3 {
        assert!(colors.iter().any(|c| c[channel] == 1.0));
        assert!(colors.iter().any(|c| c[channel] < 0.2));
    }
    let hole = world.spawn();
    world.insert(hole, BlackHole { remaining: 2.0 });
    world.insert(hole, Position(Vec2::ZERO));
    world.insert(hole, Transform::default());
    let config = world
        .get_resource::<tungsten::core::ParticleConfigRegistry>()
        .unwrap()
        .id_for_name("ex10_black_hole")
        .unwrap();
    world.insert(hole, ParticleEmitter::new(config));
    world.insert(hole, tungsten::core::ParticleEmitterState::default());
    harness.step(1);
    let world = harness.world_mut();
    crate::gameplay::scene_effects(world);
    let mut checked = 0;
    for (e, p) in world
        .query::<Particle>()
        .filter(|(_, p)| p.emitter == Some(hole))
    {
        let delta = world.get::<Transform>(e).unwrap().position;
        assert!((135.0..176.0).contains(&delta.length()));
        assert!(delta.dot(p.velocity) < 0.0);
        assert!(delta.perp_dot(p.velocity) > 0.0);
        checked += 1;
    }
    assert!(checked > 0);
    // The discrete integration must keep shrinking the orbit near the core.
    let tracked = world
        .query::<Particle>()
        .find(|(_, p)| p.emitter == Some(hole))
        .unwrap()
        .0;
    world.get_mut::<Particle>(tracked).unwrap().age = 0.1;
    world.get_mut::<Particle>(tracked).unwrap().lifetime = 2.0;
    for dt in [1.0 / 30.0, 1.0 / 60.0, 1.0 / 144.0] {
        set_dt(&mut harness, dt);
        let world = harness.world_mut();
        world.get_mut::<Transform>(tracked).unwrap().position = Vec2::new(30.0, 0.0);
        crate::gameplay::scene_effects(world);
        harness.step(1);
        let radius = harness
            .world()
            .get::<Transform>(tracked)
            .unwrap()
            .position
            .length();
        assert!((radius - (30.0 - 105.0 * dt)).abs() < 0.001);
    }
}

#[test]
fn pit_contains_two_thousand_mixed_balls_and_fast_wall_impacts() {
    let mut harness = platformer_harness(&PHYSICS_SYSTEMS);
    let world = harness.world_mut();
    seed_level(world);
    world
        .get_resource_mut::<tungsten::physics::PhysicsConfig>()
        .unwrap()
        .broadphase_cell_size = TILE;
    for i in 0..2048 {
        let small = i % 2 == 0;
        let e = impact_ball(world, Vec2::ZERO, small);
        let radius = crate::gameplay::ball_radius(world, e);
        world.insert(e, Collider::circle(radius));
        world.insert(e, RigidBody::dynamic().with_restitution(BALL_RESTITUTION));
        world.get_mut::<Position>(e).unwrap().0 = Vec2::new(
            132.0 * TILE + 48.0 + (i % 96) as f32 * 31.0,
            46.0 * TILE - 16.0 - (i / 96) as f32 * 31.0,
        );
        if i < 2 {
            world.get_mut::<Velocity>(e).unwrap().0.x = if small { -1800.0 } else { 1800.0 };
        }
    }
    for _ in 0..180 {
        harness.step(1);
        let world = harness.world();
        for (e, _) in world.query::<Ball>() {
            let p = world.get::<Position>(e).unwrap().0;
            // Check containment, allowing the solver’s shallow contact penetration.
            assert!(
                p.x >= 132.0 * TILE && p.x <= 180.0 * TILE,
                "wall escape {p}"
            );
            assert!(
                p.y >= 18.0 * TILE && p.y <= 46.0 * TILE,
                "vertical escape {p}"
            );
        }
    }
    assert_eq!(harness.world().query::<Ball>().count(), 2048);
}
