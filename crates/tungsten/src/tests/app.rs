use super::{
    App, FrameClock, FrameEnd, RedrawSchedule, compose_post_stack, format_perf_physics_line,
    format_perf_systems_line, frame_dt_secs, frame_interval_ms, is_reload_root,
    manifest_reload_roots, redraw_schedule, resolve_startup_display, runtime_display_mode,
};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tungsten_core::post::{FadeParams, PostPass, PostStack};
use tungsten_core::{
    AudioCommand, AudioCommands, CollisionEvent, CommandBuffer, Config, DeltaTime, DisplayMode,
    DisplayState, EventQueue, InputState, KeyCode, ShakeEvent, SquashEvent,
};
use tungsten_render::QuadInstance;

#[derive(Debug, Clone, Copy)]
struct ExampleEvent;

#[test]
fn register_event_is_idempotent_per_type() {
    let mut app = App::new(Config::default()).expect("App::new failed");
    let initial_flushers = app.event_flushers.len();

    app.register_event::<ExampleEvent>();
    let after_first = app.event_flushers.len();
    app.register_event::<ExampleEvent>();

    assert_eq!(after_first, initial_flushers + 1);
    assert_eq!(app.event_flushers.len(), after_first);
    assert!(
        app.world
            .get_resource::<EventQueue<ExampleEvent>>()
            .is_some()
    );
}

#[test]
fn collision_event_is_pre_registered() {
    let mut app = App::new(Config::default()).expect("App::new failed");
    let initial_flushers = app.event_flushers.len();

    app.register_event::<CollisionEvent>();

    assert_eq!(app.event_flushers.len(), initial_flushers);
}

#[test]
fn m30_game_feel_events_are_pre_registered() {
    let app = App::new(Config::default()).expect("App::new failed");
    assert!(app.world.get_resource::<EventQueue<ShakeEvent>>().is_some());
    assert!(
        app.world
            .get_resource::<EventQueue<SquashEvent>>()
            .is_some()
    );
}

#[test]
fn default_sprite_extract_installed_when_not_set() {
    let mut app = App::new(Config::default()).expect("App::new failed");
    assert!(app.extract_sprites.is_none());
    app.install_default_extracts();
    assert!(app.extract_sprites.is_some());
}

#[test]
fn user_extract_sprites_overrides_default() {
    use tungsten_core::assets::{FilterMode, TextureHandle};
    use tungsten_render::{SpriteBatch, SpriteInstance};

    let mut app = App::new(Config::default()).expect("App::new failed");
    app.set_extract_sprites(|_| {
        let mut batch = SpriteBatch::new(TextureHandle(42), FilterMode::Linear);
        batch.instances = vec![SpriteInstance::whole(
            [1.5, 2.5],
            [3.0, 4.0],
            0.25,
            [1, 2, 3, 4],
        )];
        vec![batch]
    });

    app.install_default_extracts();

    let batches = app.extract_sprites.as_ref().expect("extract_sprites set")(&app.world);
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].texture, TextureHandle(42));
    assert_eq!(batches[0].instances[0].color, [1, 2, 3, 4]);
}

#[test]
fn startup_display_downgrades_invalid_resolution_to_engine_defaults() {
    let mut config = Config::default();
    config.window.width = 0;

    let resolved = resolve_startup_display(&config);
    assert_eq!(resolved, DisplayState::default());
}

#[test]
fn runtime_display_mode_downgrades_exclusive_fullscreen() {
    assert_eq!(
        runtime_display_mode(DisplayMode::ExclusiveFullscreen),
        DisplayMode::BorderlessFullscreen
    );
    assert_eq!(
        runtime_display_mode(DisplayMode::Windowed),
        DisplayMode::Windowed
    );
}

#[test]
fn capped_frame_defers_its_redraw_to_the_frame_budget() {
    let frame_start = Instant::now();
    let budget = Duration::from_millis(50);
    assert_eq!(
        redraw_schedule(Some(budget), frame_start),
        RedrawSchedule::At(frame_start + budget)
    );
}

#[test]
fn uncapped_frame_redraws_immediately() {
    assert_eq!(
        redraw_schedule(None, Instant::now()),
        RedrawSchedule::Immediate
    );
}

#[test]
fn frame_interval_spans_two_frame_starts() {
    let first = Instant::now();
    let second = first + Duration::from_micros(16_670);
    assert_eq!(frame_interval_ms(None, first), None);
    let interval = frame_interval_ms(Some(first), second).unwrap();
    assert!((interval - 16.67).abs() < 1e-3, "interval {interval}");
}

#[test]
fn frame_dt_caps_a_long_stall() {
    // 0.1 s is the step `physics_tunneling.rs` pins as safe for an awake pile.
    assert_eq!(frame_dt_secs(Duration::from_secs(2), false), 0.1);
}

#[test]
fn frame_dt_passes_a_normal_frame_through() {
    let dt = frame_dt_secs(Duration::from_millis(16), false);
    assert!((dt - 0.016).abs() < 1e-6, "dt {dt}");
}

#[test]
fn smoke_frame_dt_is_pinned_whatever_elapsed() {
    for elapsed in [
        Duration::ZERO,
        Duration::from_millis(16),
        Duration::from_secs(2),
    ] {
        assert_eq!(frame_dt_secs(elapsed, true), 1.0 / 60.0);
    }
}

#[test]
fn reload_roots_are_the_manifest_roots_when_the_app_declares_any() {
    let roots = vec![
        PathBuf::from("assets/manifest.json"),
        PathBuf::from("examples/01_platformer/assets/manifest.json"),
    ];
    // The path given to `enable_hot_reload` does not narrow the set.
    assert_eq!(manifest_reload_roots(&roots, Some(&roots[1])), roots);
    assert_eq!(manifest_reload_roots(&roots, None), roots);
}

#[test]
fn reload_roots_fall_back_to_the_hot_reload_manifest() {
    let manifest = Path::new("assets/manifest.json");
    assert_eq!(
        manifest_reload_roots(&[], Some(manifest)),
        vec![manifest.to_path_buf()]
    );
    assert!(manifest_reload_roots(&[], None).is_empty());
}

#[test]
fn an_edit_to_any_root_manifest_routes_to_the_manifest_reload() {
    let dir = std::env::temp_dir().join(format!("tungsten_reload_roots_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for sub in ["shared", "local"] {
        std::fs::create_dir_all(dir.join(sub)).unwrap();
        std::fs::write(dir.join(sub).join("manifest.json"), "{}").unwrap();
    }
    std::fs::write(dir.join("local").join("walk.json"), "{}").unwrap();

    // Roots as an app writes them: not canonical.
    let roots = vec![
        dir.join("shared").join("manifest.json"),
        dir.join("local")
            .join("..")
            .join("local")
            .join("manifest.json"),
    ];
    let canonical = |path: PathBuf| path.canonicalize().unwrap();

    for sub in ["shared", "local"] {
        assert!(
            is_reload_root(&canonical(dir.join(sub).join("manifest.json")), &roots),
            "an edit to the {sub} root must reload the manifest"
        );
    }
    assert!(!is_reload_root(
        &canonical(dir.join("local").join("walk.json")),
        &roots
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn audio_commands_are_discarded_without_an_output_device() {
    let mut app = App::new(Config::default()).unwrap();
    app.world
        .get_resource_mut::<tungsten_core::AudioCommands>()
        .unwrap()
        .stop_all();
    app.stage_audio();
    assert!(
        app.world
            .get_resource_mut::<tungsten_core::AudioCommands>()
            .unwrap()
            .drain()
            .is_empty()
    );
}

#[test]
fn perf_systems_line_keeps_order_and_sanitizes_names() {
    let timings = vec![
        ("physics_step".to_string(), 1.234),
        ("my system=2".to_string(), 0.5),
    ];
    assert_eq!(
        format_perf_systems_line(&timings),
        "systems: physics_step=1.23ms my_system_2=0.50ms"
    );
}

#[test]
fn perf_physics_line_lists_counts_in_parser_order() {
    assert_eq!(
        format_perf_physics_line(&tungsten_core::physics::PhysicsBuffers::default()),
        "physics: proxies=0 dynamic=0 sleeping=0 pairs=0 contacts=0"
    );
}

fn half_fade() -> PostPass {
    PostPass::Fade(FadeParams {
        progress: 0.5,
        color: [0.0, 0.0, 0.0, 1.0],
    })
}

#[test]
fn transition_pass_is_appended_after_user_stack() {
    let mut user = PostStack::new();
    user.push(PostPass::Pixelate(4.0));
    user.push(PostPass::Pixelate(2.0));
    // Last frame's passes must not survive in the scratch.
    let mut scratch = PostStack::new();
    scratch.push(PostPass::Pixelate(99.0));

    let composed = compose_post_stack(&user, Some(half_fade()), &mut scratch);
    assert_eq!(
        composed.0,
        [
            PostPass::Pixelate(4.0),
            PostPass::Pixelate(2.0),
            half_fade()
        ]
    );
    assert_eq!(user.len(), 2, "the user's stack is not edited");

    // An empty user stack draws the transition pass alone.
    let empty = PostStack::new();
    let composed = compose_post_stack(&empty, Some(half_fade()), &mut scratch);
    assert_eq!(composed.0, [half_fade()]);
}

#[test]
fn no_transition_leaves_user_stack_untouched() {
    let mut user = PostStack::new();
    user.push(PostPass::Pixelate(4.0));
    let mut scratch = PostStack::new();

    let composed = compose_post_stack(&user, None, &mut scratch);
    assert!(
        std::ptr::eq(composed, &raw const user),
        "the user's stack is drawn as is"
    );
    assert_eq!(composed.0, [PostPass::Pixelate(4.0)]);
    assert!(scratch.is_empty());
}

/// One frame on an app with no window, renderer or audio device, at 60 Hz.
fn headless_frame(app: &mut App) -> FrameEnd {
    app.run_frame(Instant::now(), FrameClock::Pinned(1.0 / 60.0), |_| {})
}

/// The dt the last frame's systems read.
struct SeenDt(f32);

#[test]
fn a_pinned_dt_reaches_systems_as_given() {
    let mut app = App::new(Config::default()).unwrap();
    app.world.insert_resource(SeenDt(0.0));
    app.add_system(|world| {
        let dt = world.get_resource::<DeltaTime>().unwrap().dt;
        world.get_resource_mut::<SeenDt>().unwrap().0 = dt;
    });
    let end = app.run_frame(Instant::now(), FrameClock::Pinned(0.25), |_| {});
    assert_eq!(end, FrameEnd::Completed);
    assert_eq!(app.world.get_resource::<SeenDt>().unwrap().0, 0.25);
}

#[test]
fn a_command_a_system_queues_is_applied_by_the_end_of_its_frame() {
    struct Marker;
    let mut app = App::new(Config::default()).unwrap();
    let mut queued = false;
    app.add_system(move |world| {
        if !queued {
            queued = true;
            let cmds = world.get_resource_mut::<CommandBuffer>().unwrap();
            let pending = cmds.spawn();
            cmds.insert_pending(pending, Marker);
        }
    });
    let before = app.world.entity_count();
    headless_frame(&mut app);
    assert_eq!(app.world.entity_count(), before + 1);
    assert_eq!(app.world.query::<Marker>().count(), 1);
}

#[test]
fn a_registered_event_rotates_out_after_the_next_frame() {
    let mut app = App::new(Config::default()).unwrap();
    app.register_event::<ExampleEvent>();
    let mut sent = false;
    app.add_system(move |world| {
        if !sent {
            sent = true;
            world
                .get_resource_mut::<EventQueue<ExampleEvent>>()
                .unwrap()
                .send(ExampleEvent);
        }
    });
    headless_frame(&mut app);
    let queue = app
        .world
        .get_resource::<EventQueue<ExampleEvent>>()
        .unwrap();
    assert_eq!(queue.len(), 1);
    assert_eq!(queue.iter_current().count(), 0);
    headless_frame(&mut app);
    assert!(
        app.world
            .get_resource::<EventQueue<ExampleEvent>>()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn inspect_sees_the_frames_user_quad_extract() {
    let mut app = App::new(Config::default()).unwrap();
    app.set_extract_quads(|_| {
        vec![QuadInstance {
            position: [1.0, 2.0],
            size: [3.0, 4.0],
            color: [1.0, 0.5, 0.25, 1.0],
        }]
    });
    let mut seen = Vec::new();
    app.run_frame(Instant::now(), FrameClock::Pinned(1.0 / 60.0), |extract| {
        seen.clone_from(&extract.quads);
    });
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].position, [1.0, 2.0]);
    assert_eq!(seen[0].size, [3.0, 4.0]);
}

#[test]
fn audio_a_frame_queues_lands_in_the_capture_list_only_when_one_is_set() {
    let mut app = App::new(Config::default()).unwrap();
    app.add_system(|world| {
        world
            .get_resource_mut::<AudioCommands>()
            .unwrap()
            .stop_all();
    });

    headless_frame(&mut app);
    assert!(app.audio_capture.is_none());
    assert!(
        app.world
            .get_resource_mut::<AudioCommands>()
            .unwrap()
            .drain()
            .is_empty()
    );

    app.audio_capture = Some(Vec::new());
    headless_frame(&mut app);
    assert!(matches!(
        app.audio_capture.as_deref(),
        Some([AudioCommand::StopAll])
    ));
}

#[test]
fn a_pressed_engine_exit_binding_ends_the_frame_after_update() {
    let mut app = App::new(Config::default()).unwrap();
    app.world
        .get_resource_mut::<InputState>()
        .unwrap()
        .key_down(KeyCode::Escape);
    assert_eq!(headless_frame(&mut app), FrameEnd::ExitRequested);
}
