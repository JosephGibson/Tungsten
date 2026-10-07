use super::Harness;
use crate::App;
use crate::particles::ParticleBurstEmitted;
use crate::state::{GameState, StateContext, StateId, StateStack};
use crate::transition::{Transition, TransitionEffect};
use glam::Vec2;
use std::path::PathBuf;
use std::sync::Arc;
use tungsten_core::assets::{
    AudioHandle, BlendMode, EmissionKind, FilterMode, InitialVelocity, ParticleConfig,
    ParticleConfigRegistry, ParticleRender, Range, TextureHandle, UvRect,
};
use tungsten_core::{
    ActionMap, AssetRegistry, AudioCommand, AudioCommands, CameraController, CameraMode,
    CameraState, Collider, Config, Easing, Entity, InputState, KeyCode, Particle, ParticleEmitter,
    ParticleEmitterState, PhysicsConfig, Position, RigidBody, Sprite, Time, Transform, Tween,
    TweenChannel, TweenComplete, Velocity, Visibility, With, World,
};
use tungsten_render::{QuadInstance, TextSection};

fn app() -> App {
    App::new(Config::default()).expect("App::new failed")
}

/// The dt each frame's systems read, in frame order.
#[derive(Default)]
struct SeenDts(Vec<f32>);

fn record_dts(app: &mut App) {
    app.world_mut().insert_resource(SeenDts::default());
    app.add_system(|world| {
        let dt = world.get_resource::<Time>().unwrap().delta();
        world.get_resource_mut::<SeenDts>().unwrap().0.push(dt);
    });
}

fn burst_config() -> ParticleConfig {
    ParticleConfig {
        sprite: "spark".into(),
        render: ParticleRender::Quad,
        max_alive: 64,
        seed: Some(42),
        blend: BlendMode::Alpha,
        emission: EmissionKind::Burst {
            count: 8,
            once: true,
        },
        lifetime: Range { min: 0.5, max: 0.5 },
        initial_velocity: InitialVelocity::Radial {
            speed: Range::single(10.0),
        },
        gravity: [0.0, 0.0],
        drag_per_sec: 0.0,
        angular_velocity: Range::single(0.0),
        start_scale: Range::single(1.0),
        scale_over_life: None,
        color_over_life: None,
        alpha_over_life: None,
        tint: [1.0, 1.0, 1.0, 1.0],
    }
}

#[test]
fn frames_run_particles_tweens_the_command_flush_and_event_rotation() {
    let mut app = app();
    let world = app.world_mut();
    let config = world
        .get_resource_mut::<ParticleConfigRegistry>()
        .unwrap()
        .register(
            "burst".into(),
            PathBuf::from("/tmp/burst.json"),
            Arc::new(burst_config()),
        );
    let emitter = world.spawn();
    world.insert(emitter, ParticleEmitter::new(config));
    world.insert(emitter, ParticleEmitterState::default());
    world.insert(emitter, Transform::from_position(Vec2::ZERO));
    let test_sprite = world
        .get_resource_mut::<AssetRegistry>()
        .unwrap()
        .intern_sprite("test");
    let faded = world.spawn();
    world.insert(faded, Transform::default());
    world.insert(faded, Sprite::new(test_sprite));
    world.insert(faded, Visibility::default());
    world.insert(
        faded,
        Tween::new(0.04, Easing::Linear).with_channel(TweenChannel::ColorA { from: 0, to: 255 }),
    );

    let mut harness = Harness::new(app);
    harness.set_dt(0.05);
    assert_eq!(harness.step(1), 1);

    // The particle stage queued the burst; the command flush spawned it.
    assert_eq!(
        harness
            .world()
            .query_filtered::<Entity, With<Particle>>()
            .count(),
        8
    );
    let bursts: Vec<u32> = harness
        .events::<ParticleBurstEmitted>()
        .map(|burst| burst.count)
        .collect();
    assert_eq!(bursts, vec![8]);
    // The tween stage finished the fade; the flush removed the tween.
    assert_eq!(harness.world().get::<Sprite>(faded).unwrap().color[3], 255);
    assert!(harness.world().get::<Tween>(faded).is_none());
    assert_eq!(
        harness
            .events::<TweenComplete>()
            .filter(|done| done.entity == faded)
            .count(),
        1
    );

    // The next frame's event flush rotates both out.
    harness.step(1);
    assert_eq!(harness.events::<ParticleBurstEmitted>().count(), 0);
    assert_eq!(harness.events::<TweenComplete>().count(), 0);
    assert_eq!(
        harness
            .world()
            .query_filtered::<Entity, With<Particle>>()
            .count(),
        8
    );
}

#[test]
fn draw_holds_what_the_frame_would_draw() {
    let mut app = app();
    let world = app.world_mut();
    let quad = world
        .get_resource_mut::<AssetRegistry>()
        .unwrap()
        .register_sprite(
            "quad".into(),
            FilterMode::Nearest,
            16,
            16,
            PathBuf::from("test/quad.png"),
            TextureHandle(0),
            UvRect::FULL,
            None,
            None,
            None,
        );
    let sprite = world.spawn();
    world.insert(
        sprite,
        Transform {
            position: Vec2::new(5.0, 7.0),
            rotation: 0.0,
            scale: Vec2::new(2.0, 3.0),
        },
    );
    world.insert(sprite, Sprite::new(quad));
    world.insert(sprite, Visibility::default());
    app.set_extract_quads(|_| {
        vec![QuadInstance {
            position: [1.0, 2.0],
            size: [3.0, 4.0],
            color: [1.0, 0.5, 0.25, 1.0],
        }]
    });
    app.set_extract_text(|_| {
        vec![TextSection {
            content: "score 3".into(),
            font_id: "ui".into(),
            font_size: 16.0,
            line_height: 20.0,
            color: [255; 4],
            position: [10.0, 20.0],
            bounds: None,
            ..Default::default()
        }]
    });

    let mut harness = Harness::new(app);
    assert!(harness.draw().is_none());
    harness.step(1);
    let draw = harness.draw().expect("a completed frame");
    assert_eq!(draw.sprites.len(), 1);
    assert_eq!(draw.sprites[0].instances.len(), 1);
    assert_eq!(draw.sprites[0].instances[0].position, [5.0, 7.0]);
    assert_eq!(draw.sprites[0].instances[0].size, [32.0, 48.0]);
    assert_eq!(draw.quads.len(), 1);
    assert_eq!(draw.quads[0].position, [1.0, 2.0]);
    assert!(draw.text.iter().any(|section| section.content == "score 3"));

    // The next frame draws the sprite where the world now has it.
    harness
        .world_mut()
        .get_mut::<Transform>(sprite)
        .unwrap()
        .position = Vec2::new(40.0, 50.0);
    harness.step(1);
    let draw = harness.draw().unwrap();
    assert_eq!(draw.sprites[0].instances[0].position, [40.0, 50.0]);
}

#[test]
fn a_pinned_dt_reaches_systems_as_given_above_the_window_cap() {
    let mut app = app();
    record_dts(&mut app);
    let mut harness = Harness::new(app);
    harness.step(1);
    harness.set_dt(0.25);
    harness.step(2);
    assert_eq!(
        harness.world().get_resource::<SeenDts>().unwrap().0,
        vec![1.0 / 60.0, 0.25, 0.25]
    );
}

/// What a frame's systems read from input.
#[derive(Debug, PartialEq)]
struct InputReading {
    jump_just_pressed: bool,
    jump_held: bool,
    cursor: Option<(f32, f32)>,
}

#[derive(Default)]
struct SeenInput(Vec<InputReading>);

#[test]
fn press_action_gives_one_just_pressed_frame_and_set_cursor_reaches_input() {
    let mut app = app();
    app.world_mut().insert_resource(SeenInput::default());
    app.add_system(|world| {
        let input = world.get_resource::<InputState>().unwrap();
        let actions = world.get_resource::<ActionMap>().unwrap();
        let seen = InputReading {
            jump_just_pressed: actions.just_pressed(input, "jump"),
            jump_held: actions.is_pressed(input, "jump"),
            cursor: input.cursor_position(),
        };
        world.get_resource_mut::<SeenInput>().unwrap().0.push(seen);
    });
    let mut harness = Harness::new(app);
    harness.press_action("jump");
    harness.set_cursor(120.0, 80.0);
    harness.step(2);
    harness.release_action("jump");
    harness.step(1);
    assert_eq!(
        harness.world().get_resource::<SeenInput>().unwrap().0,
        [(true, true), (false, true), (false, false)].map(|(jump_just_pressed, jump_held)| {
            InputReading {
                jump_just_pressed,
                jump_held,
                cursor: Some((120.0, 80.0)),
            }
        })
    );
}

#[test]
fn an_engine_exit_press_stops_step() {
    let mut exiting = app();
    let mut calls = 0;
    exiting.add_system(move |world| {
        calls += 1;
        if calls == 3 {
            world
                .get_resource_mut::<InputState>()
                .unwrap()
                .key_down(KeyCode::Escape);
        }
    });
    let mut harness = Harness::new(exiting);
    assert_eq!(harness.step(5), 2);
    assert!(harness.exit_requested());
    assert_eq!(harness.frame(), 2);
    assert_eq!(harness.step(1), 0);

    let mut harness = Harness::new(app());
    harness.press_action("engine_exit");
    assert_eq!(harness.step(1), 0);
    assert!(harness.exit_requested());
}

#[test]
fn audio_holds_the_last_frames_commands() {
    let mut app = app();
    let mut played = false;
    app.add_system(move |world| {
        if !played {
            played = true;
            let audio = world.get_resource_mut::<AudioCommands>().unwrap();
            audio.play(AudioHandle(3));
            audio.stop_all();
        }
    });
    let mut harness = Harness::new(app);
    assert!(harness.audio().is_empty());
    harness.step(1);
    assert!(matches!(
        harness.audio(),
        [
            AudioCommand::Play {
                handle: AudioHandle(3),
                looping: false,
                ..
            },
            AudioCommand::StopAll
        ]
    ));
    harness.step(1);
    assert!(harness.audio().is_empty());
}

/// A state with empty hooks, for the stack to hold.
struct Plain(StateId);

impl GameState for Plain {
    fn id(&self) -> StateId {
        self.0
    }

    fn on_enter(&mut self, _ctx: &mut StateContext) {}

    fn on_exit(&mut self, _ctx: &mut StateContext) {}

    fn update(&mut self, _world: &mut World) {}
}

/// What a paused game clock must hold.
#[derive(Debug, PartialEq)]
struct Held {
    tween_x: f32,
    particles: Vec<(Entity, Vec2)>,
    body: Vec2,
}

fn held(world: &World, tweened: Entity, body: Entity) -> Held {
    Held {
        tween_x: world.get::<Transform>(tweened).unwrap().position.x,
        particles: world
            .query_filtered::<Entity, With<Particle>>()
            .collect::<Vec<_>>()
            .into_iter()
            .map(|particle| (particle, world.get::<Transform>(particle).unwrap().position))
            .collect(),
        body: world.get::<Position>(body).unwrap().0,
    }
}

#[test]
fn clock_a_pause_holds_the_game_while_a_fade_pushes_a_state() {
    let mut app = app();
    let world = app.world_mut();
    world.get_resource_mut::<PhysicsConfig>().unwrap().gravity = Vec2::new(0.0, 900.0);
    world
        .get_resource_mut::<StateStack>()
        .unwrap()
        .request_push(Plain("first"));
    let tweened = world.spawn();
    world.insert(tweened, Transform::default());
    world.insert(
        tweened,
        Tween::new(10.0, Easing::Linear).with_channel(TweenChannel::PositionX {
            from: 0.0,
            to: 100.0,
        }),
    );
    let stream = ParticleConfig {
        emission: EmissionKind::Continuous { rate_hz: 120.0 },
        lifetime: Range::single(10.0),
        ..burst_config()
    };
    let config = world
        .get_resource_mut::<ParticleConfigRegistry>()
        .unwrap()
        .register(
            "stream".into(),
            PathBuf::from("/tmp/stream.json"),
            Arc::new(stream),
        );
    let emitter = world.spawn();
    world.insert(emitter, ParticleEmitter::new(config));
    world.insert(emitter, ParticleEmitterState::default());
    world.insert(emitter, Transform::from_position(Vec2::ZERO));
    let start = Vec2::new(200.0, 100.0);
    let body = world.spawn();
    world.insert(body, Position(start));
    world.insert(body, Velocity(Vec2::ZERO));
    world.insert(body, Collider::aabb(Vec2::new(8.0, 8.0)));
    world.insert(body, RigidBody::dynamic());
    world.insert(body, Transform::from_position(start));
    let camera = world.get_resource_mut::<CameraController>().unwrap();
    camera.mode = CameraMode::Follow(body);
    camera.smoothing_factor = 1.0;
    camera.shake_max_offset = Vec2::new(8.0, 8.0);
    camera.shake_frequency_hz = 7.0;
    camera.shake_trauma = 1.0;
    camera.shake_decay = 0.5;

    let mut harness = Harness::new(app);
    harness.step(10);
    let running = held(harness.world(), tweened, body);
    assert!(running.tween_x > 0.0);
    assert!(!running.particles.is_empty());
    assert_ne!(running.body, start);

    let world = harness.world_mut();
    world.get_resource_mut::<Time>().unwrap().pause();
    let fade = Transition::new(
        TransitionEffect::Fade {
            color: [0.0, 0.0, 0.0, 1.0],
        },
        0.1,
    );
    assert!(
        world
            .get_resource_mut::<StateStack>()
            .unwrap()
            .request_push_transition(Plain("second"), fade)
    );
    // The first paused frame draws the shake phase the last running frame
    // advanced to; from then on the camera holds too.
    let mut camera_held = None;
    for frame in 1..=30 {
        harness.step(1);
        assert_eq!(
            held(harness.world(), tweened, body),
            running,
            "paused frame {frame}"
        );
        let camera = harness.world().get_resource::<CameraState>().unwrap();
        let first = *camera_held.get_or_insert(camera.position);
        assert_eq!(camera.position, first, "paused frame {frame}");
    }
    let stack = harness.world().get_resource::<StateStack>().unwrap();
    assert_eq!(stack.active_id(), Some("second"));
    assert!(!stack.is_transitioning());
}

#[test]
fn clock_half_scale_finishes_a_tween_on_twice_the_frames() {
    for (scale, finish) in [(1.0, 16), (0.5, 32)] {
        let mut app = app();
        let world = app.world_mut();
        world.get_resource_mut::<Time>().unwrap().set_scale(scale);
        let tweened = world.spawn();
        world.insert(tweened, Transform::default());
        world.insert(
            tweened,
            Tween::new(0.25, Easing::Linear)
                .with_channel(TweenChannel::PositionX { from: 0.0, to: 1.0 }),
        );
        let mut harness = Harness::new(app);
        // 1/64 s and 1/128 s are exact in binary, so the elapsed time reaches
        // 0.25 s exactly.
        harness.set_dt(1.0 / 64.0);
        let mut finished = None;
        while finished.is_none() && harness.frame() < 40 {
            harness.step(1);
            if harness
                .events::<TweenComplete>()
                .any(|done| done.entity == tweened)
            {
                finished = Some(harness.frame());
            }
        }
        assert_eq!(finished, Some(finish), "scale {scale}");
    }
}

#[test]
fn set_dt_accepts_zero() {
    let mut app = app();
    record_dts(&mut app);
    let mut harness = Harness::new(app);
    harness.set_dt(0.0);
    harness.step(1);
    assert_eq!(
        harness.world().get_resource::<SeenDts>().unwrap().0,
        vec![0.0]
    );
}

#[test]
#[should_panic(expected = "dt must be finite and not negative")]
fn set_dt_rejects_nan() {
    Harness::new(app()).set_dt(f32::NAN);
}

#[test]
#[should_panic(expected = "dt must be finite and not negative")]
fn set_dt_rejects_infinity() {
    Harness::new(app()).set_dt(f32::INFINITY);
}

#[test]
#[should_panic(expected = "dt must be finite and not negative")]
fn set_dt_rejects_a_negative_dt() {
    Harness::new(app()).set_dt(-0.01);
}
