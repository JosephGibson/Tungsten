use super::Harness;
use crate::App;
use crate::particles::ParticleBurstEmitted;
use glam::Vec2;
use std::path::PathBuf;
use std::sync::Arc;
use tungsten_core::assets::{
    AudioHandle, BlendMode, EmissionKind, FilterMode, InitialVelocity, ParticleConfig,
    ParticleConfigRegistry, ParticleRender, Range, TextureHandle, UvRect,
};
use tungsten_core::{
    ActionMap, AssetRegistry, AudioCommand, AudioCommands, Config, DeltaTime, Easing, InputState,
    KeyCode, Particle, ParticleEmitter, ParticleEmitterState, Sprite, Transform, Tween,
    TweenChannel, TweenComplete, Visibility,
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
        let dt = world.get_resource::<DeltaTime>().unwrap().dt;
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
    let faded = world.spawn();
    world.insert(faded, Transform::default());
    world.insert(faded, Sprite::new("test"));
    world.insert(faded, Visibility::default());
    world.insert(
        faded,
        Tween::new(0.04, Easing::Linear).with_channel(TweenChannel::ColorA { from: 0, to: 255 }),
    );

    let mut harness = Harness::new(app);
    harness.set_dt(0.05);
    assert_eq!(harness.step(1), 1);

    // The particle stage queued the burst; the command flush spawned it.
    assert_eq!(harness.world().query_entities::<Particle>().len(), 8);
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
    assert_eq!(harness.world().query_entities::<Particle>().len(), 8);
}

#[test]
fn draw_holds_what_the_frame_would_draw() {
    let mut app = app();
    let world = app.world_mut();
    world
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
    world.insert(sprite, Sprite::new("quad"));
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
