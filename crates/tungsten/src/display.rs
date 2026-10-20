use std::time::Duration;

use crate::app::WindowSize;
use crate::telemetry::DisplayTelemetry;
use tungsten_core::{
    ActionMap, DisplayMode, DisplayState, DisplayValidationError, InputState, MonitorInfo,
    Resolution, ScaleFactor, VideoMode, World,
};
use winit::window::Window;

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
    /// What `requested` changes from `current`; the mode as it runs, so an
    /// exclusive request over borderless changes no mode (M59).
    pub(crate) fn between(current: &DisplayState, requested: &DisplayState) -> Self {
        Self {
            resize: current.resolution != requested.resolution,
            display_mode_changed: current.display_mode
                != runtime_display_mode(requested.display_mode),
            surface_pacing_changed: current.vsync != requested.vsync
                || current.present_mode != requested.present_mode
                || current.max_frame_latency != requested.max_frame_latency,
            scale_mode_changed: current.scale_mode != requested.scale_mode,
            frame_rate_cap_changed: current.frame_rate_cap != requested.frame_rate_cap,
        }
    }
}

/// The mode `requested` runs as: exclusive fullscreen as borderless, which
/// is all the runtime switch supports yet.
pub(crate) fn runtime_display_mode(requested: DisplayMode) -> DisplayMode {
    match requested {
        DisplayMode::ExclusiveFullscreen => DisplayMode::BorderlessFullscreen,
        other => other,
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

/// Writes `WindowSize` and the [`ScaleFactor`]'s window size together, so
/// a window-relative factor follows every resize (`D-184`).
fn set_window_size(world: &mut World, width: u32, height: u32) {
    if let Some(window_size) = world.get_resource_mut::<WindowSize>() {
        window_size.width = width;
        window_size.height = height;
    } else {
        world.insert_resource(WindowSize { width, height });
    }
    if let Some(scale) = world.get_resource_mut::<ScaleFactor>() {
        scale.set_window_size(width, height);
    }
}

pub(crate) fn sync_display_state_and_telemetry(
    world: &mut World,
    state: DisplayState,
    actual_present_mode: Option<String>,
) {
    set_window_size(world, state.resolution.width, state.resolution.height);

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

    set_window_size(world, width, height);

    let mut display_state = world
        .get_resource::<DisplayState>()
        .copied()
        .unwrap_or_default();
    display_state.resolution = Resolution { width, height };
    sync_display_state_and_telemetry(world, display_state, actual_present_mode);
}

/// The monitor `window` is on (`D-184`): its current monitor, else the
/// first the platform lists, since Wayland has no primary and knows the
/// current one only once the surface has entered an output; `None` when no
/// monitor answers or it reports a 0 side.
pub(crate) fn read_monitor(window: &Window) -> Option<MonitorInfo> {
    let monitor = window
        .current_monitor()
        .or_else(|| window.available_monitors().next())?;
    let size = monitor.size();
    let modes = monitor.video_modes().map(|mode| {
        let mode_size = mode.size();
        VideoMode::new(
            Resolution {
                width: mode_size.width,
                height: mode_size.height,
            },
            mode.refresh_rate_millihertz(),
        )
    });
    MonitorInfo::new(
        Resolution {
            width: size.width,
            height: size.height,
        },
        monitor.refresh_rate_millihertz(),
        modes,
    )
}

/// Inserts, replaces or removes the [`MonitorInfo`] resource, logging one
/// info line when it changes.
pub(crate) fn set_monitor(world: &mut World, monitor: Option<MonitorInfo>) {
    if world.get_resource::<MonitorInfo>() == monitor.as_ref() {
        return;
    }
    if let Some(monitor) = monitor {
        let rate = monitor.refresh_millihertz.map_or_else(
            || "an unknown rate".to_owned(),
            |millihertz| format!("{} Hz", f64::from(millihertz) / 1000.0),
        );
        log::info!(
            "Monitor {}×{} at {rate}, {} modes",
            monitor.size.width,
            monitor.size.height,
            monitor.modes.len()
        );
        world.insert_resource(monitor);
    } else {
        log::info!("Monitor unknown");
        world.remove_resource::<MonitorInfo>();
    }
}

#[cfg(test)]
#[path = "tests/display.rs"]
mod tests;
