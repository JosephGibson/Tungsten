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
