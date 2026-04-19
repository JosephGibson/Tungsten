use std::time::Duration;

use crate::app::WindowSize;
use crate::telemetry::DisplayTelemetry;
use tungsten_core::{
    ActionMap, DisplayMode, DisplayState, DisplayValidationError, InputState, Resolution, World,
};

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct PendingDisplay(pub(crate) Option<DisplayState>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DisplayDelta {
    pub(crate) resize: bool,
    pub(crate) display_mode_changed: bool,
    pub(crate) surface_pacing_changed: bool,
    pub(crate) scale_mode_changed: bool,
    pub(crate) frame_rate_cap_changed: bool,
}

impl DisplayDelta {
    pub(crate) fn between(current: &DisplayState, requested: &DisplayState) -> Self {
        Self {
            resize: current.resolution != requested.resolution,
            display_mode_changed: current.display_mode != requested.display_mode,
            surface_pacing_changed: current.vsync != requested.vsync
                || current.present_mode != requested.present_mode
                || current.max_frame_latency != requested.max_frame_latency,
            scale_mode_changed: current.scale_mode != requested.scale_mode,
            frame_rate_cap_changed: current.frame_rate_cap != requested.frame_rate_cap,
        }
    }
}

pub fn request_display_settings(
    world: &mut World,
    requested: DisplayState,
) -> Result<(), DisplayValidationError> {
    requested.validate()?;
    if let Some(pending) = world.get_resource_mut::<PendingDisplay>() {
        pending.0 = Some(requested);
    } else {
        world.insert_resource(PendingDisplay(Some(requested)));
    }
    Ok(())
}

/// Engine-owned display hotkeys, now routed through `ActionMap` instead of
/// hardcoded key branches. Defaults remain `F9` (vsync) and `F11`
/// (fullscreen), but `input.json` can override them.
pub(crate) fn engine_display_input_system(world: &mut World) {
    let (toggle_vsync, toggle_fullscreen) = {
        let Some(input) = world.get_resource::<InputState>() else {
            return;
        };
        let Some(actions) = world.get_resource::<ActionMap>() else {
            return;
        };
        (
            actions.just_pressed(input, "engine_toggle_vsync"),
            actions.just_pressed(input, "engine_toggle_fullscreen"),
        )
    };

    if !toggle_vsync && !toggle_fullscreen {
        return;
    }

    let current = world
        .get_resource::<DisplayState>()
        .copied()
        .unwrap_or_default();
    let mut next = current;

    if toggle_fullscreen {
        next.display_mode = match current.display_mode {
            DisplayMode::Windowed => DisplayMode::BorderlessFullscreen,
            DisplayMode::BorderlessFullscreen | DisplayMode::ExclusiveFullscreen => {
                DisplayMode::Windowed
            }
        };
    }

    if toggle_vsync {
        next.vsync = !current.vsync;
        next.present_mode = None;
    }

    if let Err(err) = request_display_settings(world, next) {
        log::error!("Display request rejected: {err}");
    }
}

pub(crate) fn take_pending_display(world: &mut World) -> Option<DisplayState> {
    world
        .get_resource_mut::<PendingDisplay>()
        .and_then(|pending| pending.0.take())
}

pub(crate) fn frame_budget_for(frame_rate_cap: Option<u32>) -> Option<Duration> {
    frame_rate_cap.map(|cap| Duration::from_secs_f64(1.0 / f64::from(cap)))
}

pub(crate) fn sync_display_state_and_telemetry(
    world: &mut World,
    state: DisplayState,
    actual_present_mode: Option<String>,
) {
    if let Some(window_size) = world.get_resource_mut::<WindowSize>() {
        window_size.width = state.resolution.width;
        window_size.height = state.resolution.height;
    } else {
        world.insert_resource(WindowSize {
            width: state.resolution.width,
            height: state.resolution.height,
        });
    }

    if let Some(display_state) = world.get_resource_mut::<DisplayState>() {
        *display_state = state;
    } else {
        world.insert_resource(state);
    }

    if let Some(telemetry) = world.get_resource_mut::<DisplayTelemetry>() {
        telemetry.apply_state(&state, actual_present_mode);
    } else {
        world.insert_resource(DisplayTelemetry::from_state(&state, actual_present_mode));
    }
}

pub(crate) fn sync_window_resolution(
    world: &mut World,
    width: u32,
    height: u32,
    actual_present_mode: Option<String>,
) {
    if width == 0 || height == 0 {
        return;
    }

    if let Some(window_size) = world.get_resource_mut::<WindowSize>() {
        window_size.width = width;
        window_size.height = height;
    } else {
        world.insert_resource(WindowSize { width, height });
    }

    let mut display_state = world
        .get_resource::<DisplayState>()
        .copied()
        .unwrap_or_default();
    display_state.resolution = Resolution { width, height };
    sync_display_state_and_telemetry(world, display_state, actual_present_mode);
}

#[cfg(test)]
mod tests {
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
}
