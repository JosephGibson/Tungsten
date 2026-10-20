use super::*;
use crate::telemetry::DisplayTelemetry;
use tungsten_core::{ActionMap, DisplayMode, InputState, KeyCode, ScaleMode};

fn seed_world() -> World {
    let mut world = World::new();
    let state = DisplayState::default();
    world.insert_resource(state);
    world.insert_resource(PendingDisplay::default());
    world.insert_resource(WindowSize {
        width: state.resolution.width,
        height: state.resolution.height,
    });
    world.insert_resource(DisplayTelemetry::from_state(&state, None));
    world
}

#[test]
fn later_request_replaces_earlier_one() {
    let mut world = seed_world();

    request_display_settings(
        &mut world,
        DisplayState {
            frame_rate_cap: Some(60),
            ..DisplayState::default()
        },
    )
    .unwrap();
    request_display_settings(
        &mut world,
        DisplayState {
            display_mode: DisplayMode::BorderlessFullscreen,
            ..DisplayState::default()
        },
    )
    .unwrap();

    let pending = take_pending_display(&mut world).unwrap();
    assert_eq!(pending.display_mode, DisplayMode::BorderlessFullscreen);
    assert_eq!(pending.frame_rate_cap, None);
}

#[test]
fn frame_budget_math_returns_expected_duration() {
    let budget = frame_budget_for(Some(120)).unwrap();
    assert_eq!(budget, Duration::from_secs_f64(1.0 / 120.0));
    assert_eq!(frame_budget_for(None), None);
}

#[test]
fn sync_helpers_keep_state_and_telemetry_in_step() {
    let mut world = seed_world();
    let state = DisplayState {
        display_mode: DisplayMode::BorderlessFullscreen,
        vsync: true,
        scale_mode: ScaleMode::Integer,
        frame_rate_cap: Some(144),
        ..DisplayState::default()
    };

    sync_display_state_and_telemetry(&mut world, state, Some("fifo".to_string()));
    sync_window_resolution(&mut world, 1920, 1080, Some("fifo".to_string()));

    let state = world.get_resource::<DisplayState>().unwrap();
    let telemetry = world.get_resource::<DisplayTelemetry>().unwrap();
    let window_size = world.get_resource::<WindowSize>().unwrap();

    assert_eq!(
        state.resolution,
        Resolution {
            width: 1920,
            height: 1080
        }
    );
    assert_eq!(telemetry.resolution, (1920, 1080));
    assert_eq!(telemetry.display_mode, DisplayMode::BorderlessFullscreen);
    assert!(telemetry.vsync);
    assert_eq!(telemetry.scale_mode, ScaleMode::Integer);
    assert_eq!(telemetry.frame_rate_cap, Some(144));
    assert_eq!(telemetry.actual_present_mode.as_deref(), Some("fifo"));
    assert_eq!(window_size.width, 1920);
    assert_eq!(window_size.height, 1080);
}

#[test]
fn engine_display_input_system_uses_engine_actions() {
    let mut world = seed_world();
    let current = world
        .get_resource::<DisplayState>()
        .copied()
        .unwrap_or_default();
    let mut input = InputState::new();
    input.key_down(KeyCode::F9);
    input.key_down(KeyCode::F11);
    world.insert_resource(input);
    world.insert_resource(ActionMap::default_map());

    engine_display_input_system(&mut world);

    let requested = take_pending_display(&mut world).unwrap();
    assert_eq!(requested.display_mode, DisplayMode::BorderlessFullscreen);
    assert_eq!(requested.vsync, !current.vsync);
    assert_eq!(requested.present_mode, None);
}

#[test]
fn an_exclusive_request_over_borderless_changes_no_mode() {
    let current = DisplayState {
        display_mode: DisplayMode::BorderlessFullscreen,
        ..DisplayState::default()
    };
    let requested = DisplayState {
        display_mode: DisplayMode::ExclusiveFullscreen,
        ..current
    };
    assert!(
        !DisplayDelta::between(&current, &requested).display_mode_changed,
        "exclusive runs as borderless, which the window already is"
    );
    let windowed = DisplayState {
        display_mode: DisplayMode::Windowed,
        ..current
    };
    assert!(DisplayDelta::between(&windowed, &requested).display_mode_changed);
}

#[test]
fn window_syncs_set_the_scale_factors_window_size() {
    let mut world = seed_world();
    let mut scale = tungsten_core::ScaleFactor::default();
    scale.set_reference_height(Some(720.0));
    world.insert_resource(scale);
    sync_window_resolution(&mut world, 3840, 2160, None);
    assert_eq!(
        world.resource::<tungsten_core::ScaleFactor>().get(),
        3.0,
        "a resize moves the window-relative factor"
    );
    sync_window_resolution(&mut world, 0, 0, None);
    assert_eq!(
        world.resource::<tungsten_core::ScaleFactor>().get(),
        3.0,
        "a 0×0 resize, a minimized window, changes nothing"
    );
    let state = DisplayState {
        resolution: Resolution {
            width: 1920,
            height: 1080,
        },
        ..DisplayState::default()
    };
    sync_display_state_and_telemetry(&mut world, state, None);
    let size = *world.resource::<WindowSize>();
    assert_eq!((size.width, size.height), (1920, 1080));
    assert_eq!(world.resource::<tungsten_core::ScaleFactor>().get(), 1.5);
}

#[test]
fn set_monitor_inserts_replaces_and_removes_it() {
    let mut world = World::new();
    let size = |width, height| Resolution { width, height };
    let monitor = MonitorInfo::new(
        size(3840, 2160),
        Some(60_000),
        [VideoMode::new(size(3840, 2160), 60_000)],
    );
    set_monitor(&mut world, monitor.clone());
    assert_eq!(world.get_resource::<MonitorInfo>(), monitor.as_ref());
    let other = MonitorInfo::new(size(1920, 1080), None, []);
    set_monitor(&mut world, other.clone());
    assert_eq!(world.get_resource::<MonitorInfo>(), other.as_ref());
    set_monitor(&mut world, None);
    assert!(!world.has_resource::<MonitorInfo>());
    set_monitor(&mut world, None);
    assert!(!world.has_resource::<MonitorInfo>());
}
