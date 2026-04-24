use std::time::Duration;

use crate::app::WindowSize;
use crate::telemetry::DisplayTelemetry;
use tungsten_core::{
    ActionMap, DisplayMode, DisplayState, DisplayValidationError, InputState, Resolution, World,
};

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct PendingDisplay(pub(crate) Option<DisplayState>);

#[allow(clippy::struct_excessive_bools)]
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

/// Display hotkeys through `ActionMap`; defaults F9/F11.
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
#[path = "tests/display.rs"]
mod tests;
