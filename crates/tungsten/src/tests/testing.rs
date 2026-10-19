use super::Harness;
use crate::app::PendingExit;
use crate::particles::ParticleBurstEmitted;
use crate::state::{GameState, StateContext, StateId, StateStack};
use crate::transition::{Transition, TransitionEffect};
use crate::{App, ExitReason, Exiting, request_exit};
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
    ParticleEmitterState, PhysicsConfig, Position, RigidBody, Sprite, Stage, Time, Transform,
    Tween, TweenChannel, TweenComplete, Velocity, Visibility, With, World, system,
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
fn press_action_presses_a_gamepad_binding() {
    use tungsten_core::{AxisDirection, Binding, GamepadAxis, GamepadButton, GamepadId};

    let mut app = app();
    let actions = app.world_mut().resource_mut::<ActionMap>();
    actions.replace_bindings(
        "jump",
        vec![Binding::GamepadButton {
            button: GamepadButton::South,
        }],
    );
    actions.replace_bindings(
        "move_left",
        vec![Binding::GamepadAxis {
            axis: GamepadAxis::LeftStickX,
            direction: AxisDirection::Negative,
        }],
    );
    #[derive(Default)]
    struct Seen(Vec<(bool, bool, f32)>);
    app.world_mut().insert_resource(Seen::default());
    app.add_system(|world| {
        let input = world.get_resource::<InputState>().unwrap();
        let actions = world.get_resource::<ActionMap>().unwrap();
        let seen = (
            actions.just_pressed(input, "jump"),
            actions.is_pressed(input, "jump"),
            actions.value(input, "move_left"),
        );
        world.get_resource_mut::<Seen>().unwrap().0.push(seen);
    });
    let mut harness = Harness::new(app);
    harness.press_action("jump");
    harness.press_action("move_left");
    harness.step(2);
    let input = harness.world().resource::<InputState>();
    assert!(input.is_gamepad_pressed_on(GamepadId::new(0), GamepadButton::South));
    assert_eq!(
        input.gamepad_axis_on(GamepadId::new(0), GamepadAxis::LeftStickX),
        -1.0,
        "full tilt its way"
    );
    harness.release_action("jump");
    harness.release_action("move_left");
    harness.step(1);
    assert_eq!(
        harness.world().resource::<Seen>().0,
        [(true, true, 1.0), (false, true, 1.0), (false, false, 0.0)]
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

/// What each stage's system saw of [`Exiting`], one entry a run.
#[derive(Default)]
struct SeenExiting {
    pre_update: Vec<Option<ExitReason>>,
    fixed_update: Vec<Option<ExitReason>>,
    update: Vec<Option<ExitReason>>,
    post_update: Vec<Option<ExitReason>>,
}

impl SeenExiting {
    fn stages(&self) -> [&Vec<Option<ExitReason>>; 4] {
        [
            &self.pre_update,
            &self.fixed_update,
            &self.update,
            &self.post_update,
        ]
    }
}

/// An app whose every stage records [`Exiting`], whose frame `n` (from 1)
/// draws one quad at x = `n` and plays `AudioHandle(n)`, and whose update
/// calls [`request_exit`] in each frame `requests` names.
fn exiting_app(requests: &'static [u64]) -> App {
    fn reason(world: &World) -> Option<ExitReason> {
        world
            .get_resource::<Exiting>()
            .map(|exiting| exiting.reason)
    }
    let mut app = app();
    app.world_mut().insert_resource(SeenExiting::default());
    app.add_system_to(
        Stage::PreUpdate,
        system("seen_pre_update", |world| {
            let seen = reason(world);
            world.resource_mut::<SeenExiting>().pre_update.push(seen);
        }),
    );
    app.add_system_to(
        Stage::FixedUpdate,
        system("seen_fixed_update", |world| {
            let seen = reason(world);
            world.resource_mut::<SeenExiting>().fixed_update.push(seen);
        }),
    );
    app.add_system(move |world| {
        let seen = reason(world);
        let frame = world.resource::<Time>().frame();
        world.resource_mut::<SeenExiting>().update.push(seen);
        world
            .resource_mut::<AudioCommands>()
            .play(AudioHandle(frame as u32));
        for _ in requests.iter().filter(|&&each| each == frame) {
            request_exit(world);
        }
    });
    app.add_system_to(
        Stage::PostUpdate,
        system("seen_post_update", |world| {
            let seen = reason(world);
            world.resource_mut::<SeenExiting>().post_update.push(seen);
        }),
    );
    app.set_extract_quads(|world| {
        vec![QuadInstance {
            position: [world.resource::<Time>().frame() as f32, 0.0],
            size: [1.0, 1.0],
            color: [1.0; 4],
        }]
    });
    app
}

#[test]
fn exiting_follows_request_exit_for_one_frame() {
    let mut harness = Harness::new(exiting_app(&[2]));
    assert_eq!(harness.step(5), 3);
    assert!(harness.exit_requested());
    let requested = Some(ExitReason::Requested);
    for stage in harness.world().resource::<SeenExiting>().stages() {
        assert_eq!(stage[..], [None, None, requested], "every stage ran in it");
    }
    assert_eq!(
        harness.world().resource::<Exiting>().reason,
        ExitReason::Requested
    );
    assert_eq!(harness.step(1), 0, "no frame runs after the last");
    assert_eq!(harness.frame(), 3);
    assert_eq!(harness.world().resource::<SeenExiting>().update.len(), 3);
}

#[test]
fn exiting_keeps_the_last_frames_draw_audio_and_count() {
    let mut harness = Harness::new(exiting_app(&[2]));
    assert_eq!(harness.step(5), 3);
    assert_eq!(harness.frame(), 3, "the last frame counts");
    let draw = harness.draw().expect("the last frame drew");
    assert_eq!(draw.quads.len(), 1);
    assert_eq!(draw.quads[0].position, [3.0, 0.0], "the last frame's draw");
    assert!(
        matches!(
            harness.audio(),
            [AudioCommand::Play {
                handle: AudioHandle(3),
                ..
            }]
        ),
        "the last frame's audio: {:?}",
        harness.audio()
    );
}

#[test]
fn exiting_follows_a_close_for_one_frame() {
    let mut harness = Harness::new(exiting_app(&[]));
    assert_eq!(harness.step(2), 2);
    harness.request_close();
    assert!(!harness.exit_requested(), "the last frame has not run yet");
    assert_eq!(harness.step(5), 1);
    assert!(harness.exit_requested());
    let closed = Some(ExitReason::CloseRequested);
    for stage in harness.world().resource::<SeenExiting>().stages() {
        assert_eq!(stage[..], [None, None, closed]);
    }
    assert_eq!(harness.step(1), 0);
    assert_eq!(harness.frame(), 3);
}

#[test]
fn exiting_ignores_a_second_request_and_one_in_the_last_frame() {
    // Frame 2 asks twice; frame 3, the last, asks again.
    let mut harness = Harness::new(exiting_app(&[2, 2, 3]));
    assert_eq!(harness.step(10), 3);
    let world = harness.world();
    assert_eq!(world.resource::<Exiting>().reason, ExitReason::Requested);
    assert!(!world.has_resource::<PendingExit>(), "nothing left pending");
    assert_eq!(world.resource::<SeenExiting>().update.len(), 3);

    // A close during the last frame keeps the request's reason.
    let mut harness = Harness::new(exiting_app(&[1]));
    assert_eq!(harness.step(1), 1);
    harness.request_close();
    assert_eq!(harness.step(5), 1);
    assert_eq!(
        harness.world().resource::<Exiting>().reason,
        ExitReason::Requested
    );
}

#[test]
fn exiting_is_absent_from_ordinary_frames() {
    let mut harness = Harness::new(exiting_app(&[]));
    assert_eq!(harness.step(600), 600);
    assert!(!harness.exit_requested());
    let seen = harness.world().resource::<SeenExiting>();
    for stage in seen.stages() {
        assert!(stage.iter().all(Option::is_none));
    }
    assert_eq!(seen.update.len(), 600);
    assert!(!harness.world().has_resource::<Exiting>());
}

#[test]
fn exiting_never_follows_engine_exit() {
    // `engine_exit` pressed in the frame that also asks to exit: the frame
    // stops after update and no last frame runs.
    let mut app = exiting_app(&[3]);
    let mut calls = 0;
    app.add_system(move |world| {
        calls += 1;
        if calls == 3 {
            world
                .get_resource_mut::<InputState>()
                .unwrap()
                .key_down(KeyCode::Escape);
        }
    });
    let mut harness = Harness::new(app);
    assert_eq!(harness.step(5), 2);
    assert!(harness.exit_requested());
    assert_eq!(harness.step(1), 0);
    let world = harness.world();
    assert!(!world.has_resource::<Exiting>());
    for stage in world.resource::<SeenExiting>().stages() {
        assert!(stage.iter().all(Option::is_none));
    }

    // Pressed in the last frame itself: that frame is cut short too.
    let mut app = exiting_app(&[1]);
    let mut calls = 0;
    app.add_system(move |world| {
        calls += 1;
        if calls == 2 {
            world
                .get_resource_mut::<InputState>()
                .unwrap()
                .key_down(KeyCode::Escape);
        }
    });
    let mut harness = Harness::new(app);
    assert_eq!(
        harness.step(5),
        1,
        "the cut-short last frame does not count"
    );
    assert!(harness.exit_requested());
    assert_eq!(
        harness.world().resource::<SeenExiting>().post_update[..],
        [None],
        "its later stages never ran"
    );
}
