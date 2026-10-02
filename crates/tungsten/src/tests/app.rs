use super::{
    App, RedrawSchedule, format_perf_physics_line, format_perf_systems_line, frame_dt_secs,
    frame_interval_ms, redraw_schedule, resolve_startup_display, runtime_display_mode,
};
use std::time::{Duration, Instant};
use tungsten_core::{
    CollisionEvent, Config, DisplayMode, DisplayState, EventQueue, ShakeEvent, SquashEvent,
};

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
