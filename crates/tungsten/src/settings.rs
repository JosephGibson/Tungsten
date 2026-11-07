//! The runtime apply of the player's [`Settings`] (`D-159`): what a settings
//! screen calls after changing them, before `Settings::save`, and the
//! display they request.

use tungsten_core::{
    AudioBus, AudioCommands, Config, ConfigError, DisplayMode, DisplayState, FrameRateCap,
    InputState, Resolution, ScaleFactor, Settings, UiTree, World,
};

use crate::app::resolve_startup_display;
use crate::display::{PendingDisplay, request_display_settings};

/// The config as `App::new` received it, before the settings: the baseline
/// every [`apply_settings`] starts from, so an unset value returns to the
/// shipped one.
pub(crate) struct ShippedConfig(pub(crate) Config);

/// The last windowed target [`apply_settings`] sent, or the resolution
/// startup resolved (`D-169`): a windowed apply sends the target only when
/// it differs, so a size the window took by hand or by a maximize stays.
pub(crate) struct WindowedTarget(pub(crate) Resolution);

/// Applies the world's [`Settings`] now, as a restart would: the display
/// mode, resolution, vsync and frame-rate limit (`D-180`) through a display
/// request, the UI scale to [`ScaleFactor`], the text scale to the
/// [`UiTree`] (M56), the master volume and every bus's volume through
/// [`AudioCommands`], and the sticks' dead zone and sensitivity to
/// [`InputState`]'s `StickResponse` (P7-12), which the UI routing passes to
/// `RoutedInput`. While `Settings::mute_unfocused` is on and the window has
/// no focus, the master volume sent is 0 and the `Settings` stay as set
/// (S-145); a focus change sends the master volume again. Bloom is read at
/// each frame's draw (S-50). A value the player unset returns to the shipped one: the
/// UI scale and the text scale to 1.0, the master volume and a bus's volume
/// to [`shipped_volume`] (`D-184`), the stick's to `StickResponse::DEFAULT`.
/// A display request queued earlier this frame, such as a hotkey's, takes
/// the persisted values, so the last apply wins; the F9 and F11 hotkeys'
/// changes to them since startup are replaced. In windowed
/// mode the resolution is sent only when the set or shipped one differs
/// from the last one sent (`D-169`), so a window resized by hand or
/// maximized keeps its size at an unrelated apply, and one set while
/// fullscreen is sent on the return to windowed. A function, not a system:
/// call it after changing `Settings`, then save them.
///
/// Without `App`'s shipped config (a `World` built by hand), unset display
/// values keep the current state. Without `Settings`, nothing happens.
pub fn apply_settings(world: &mut World) {
    let Some(settings) = world.get_resource::<Settings>().cloned() else {
        return;
    };
    // An unset UI scale applies as `tungsten.json`'s `ui.scale` (redo 7.17).
    let shipped = world
        .get_resource::<ShippedConfig>()
        .and_then(|ShippedConfig(config)| config.ui.scale);
    if let Some(scale) = world.get_resource_mut::<ScaleFactor>() {
        scale.set_ui_scale(settings.ui_scale().or(shipped).unwrap_or(1.0));
    }
    if let Some(tree) = world.get_resource_mut::<UiTree>() {
        tree.set_text_scale(settings.text_scale().unwrap_or(1.0));
    }
    if let Some(input) = world.get_resource_mut::<InputState>() {
        input.set_stick_response(settings.stick_response());
    }
    if world.has_resource::<AudioCommands>() {
        let master = master_volume(world, &settings);
        let buses: Vec<(AudioBus, f32)> = AudioBus::ALL
            .iter()
            .map(|&bus| {
                let shipped = || shipped_volume(world, Some(bus));
                (bus, settings.bus_volume(bus).unwrap_or_else(shipped))
            })
            .collect();
        let audio = world.resource_mut::<AudioCommands>();
        audio.set_master_volume(master);
        for (bus, volume) in buses {
            audio.set_bus_volume(bus, volume);
        }
    }
    request_settings_display(world);
}

/// The master volume [`apply_settings`] sends: 0 while the mute holds (the
/// window unfocused and `Settings::mute_unfocused` on), else the set or
/// [`shipped_volume`].
fn master_volume(world: &World, settings: &Settings) -> f32 {
    let unfocused = world
        .get_resource::<InputState>()
        .is_some_and(|input| !input.focused());
    if unfocused && settings.mute_unfocused() == Some(true) {
        return 0.0;
    }
    settings
        .master_volume()
        .unwrap_or_else(|| shipped_volume(world, None))
}

/// A focus change (S-145): sends the master volume [`apply_settings`] would
/// while `Settings::mute_unfocused` is on; nothing while it is off, when the
/// master volume does not follow the focus.
pub(crate) fn apply_focus_mute(world: &mut World) {
    let Some(settings) = world.get_resource::<Settings>() else {
        return;
    };
    if settings.mute_unfocused() != Some(true) {
        return;
    }
    let volume = master_volume(world, settings);
    if let Some(audio) = world.get_resource_mut::<AudioCommands>() {
        audio.set_master_volume(volume);
    }
}

/// The display [`apply_settings`] requests (S-147): the world's [`Settings`]
/// over the live display, a request queued this frame, else
/// [`DisplayState`]. With `App`'s shipped config, a value the player unset is
/// the shipped one, and a windowed size the same as the last one sent keeps
/// the live size, so a window resized by hand or maximized shows as it runs
/// (`D-169`); without it (a `World` built by hand), an unset value is the
/// live one. Without `Settings`, or when the shipped config cannot take them,
/// it is the live display.
#[must_use]
pub fn requested_display(world: &World) -> DisplayState {
    merge_display(world).map_or_else(|_| live_display(world), |(next, _)| next)
}

/// A display request queued this frame, else [`DisplayState`].
fn live_display(world: &World) -> DisplayState {
    let current = world
        .get_resource::<DisplayState>()
        .copied()
        .unwrap_or_default();
    world
        .get_resource::<PendingDisplay>()
        .and_then(|pending| pending.0)
        .unwrap_or(current)
}

/// [`requested_display`], with the windowed size to record as sent when it
/// differs from the last one.
fn merge_display(world: &World) -> Result<(DisplayState, Option<Resolution>), ConfigError> {
    let mut next = live_display(world);
    let Some(settings) = world.get_resource::<Settings>() else {
        return Ok((next, None));
    };
    let mut windowed = None;
    if let Some(ShippedConfig(shipped)) = world.get_resource::<ShippedConfig>() {
        let mut config = shipped.clone();
        config.apply_settings(settings)?;
        let target = resolve_startup_display(&config);
        next.display_mode = target.display_mode;
        next.vsync = target.vsync;
        next.present_mode = target.present_mode;
        next.frame_rate_cap = target.frame_rate_cap;
        let sent = world
            .get_resource::<WindowedTarget>()
            .map(|windowed| windowed.0);
        if target.display_mode != DisplayMode::Windowed {
            // The fullscreen path ignores it; the memory stays (`D-169`).
            next.resolution = target.resolution;
        } else if sent != Some(target.resolution) {
            next.resolution = target.resolution;
            windowed = Some(target.resolution);
        }
    } else {
        if let Some(mode) = settings.display_mode() {
            next.display_mode = mode;
        }
        if let Some(resolution) = settings.resolution() {
            next.resolution = resolution;
        }
        if let Some(vsync) = settings.vsync() {
            next.vsync = vsync;
            next.present_mode = None;
        }
        if let Some(cap) = settings.frame_rate_cap() {
            next.frame_rate_cap = match cap {
                FrameRateCap::Uncapped => None,
                FrameRateCap::Fps(fps) => Some(fps.get()),
            };
        }
    }
    Ok((next, windowed))
}

/// Requests [`requested_display`], [`apply_settings`]' display part: a
/// request equal to the live display drops one queued earlier this frame,
/// so the last apply wins.
pub(crate) fn request_settings_display(world: &mut World) {
    let current = world
        .get_resource::<DisplayState>()
        .copied()
        .unwrap_or_default();
    let (next, windowed) = match merge_display(world) {
        Ok(merged) => merged,
        Err(err) => {
            log::error!("Settings not applied: {err}");
            return;
        }
    };
    if let Some(resolution) = windowed {
        world.insert_resource(WindowedTarget(resolution));
    }
    if next == current {
        if let Some(pending) = world.get_resource_mut::<PendingDisplay>() {
            pending.0 = None;
        }
    } else if let Err(err) = request_display_settings(world, next) {
        log::error!("Display request rejected: {err}");
    }
}

/// The volume `tungsten.json`'s `audio` section ships for the master
/// (`None`) or `bus` (`D-184`), which an unset setting returns to; 1.0
/// without `App`'s shipped config (a `World` built by hand).
#[must_use]
pub fn shipped_volume(world: &World, bus: Option<AudioBus>) -> f32 {
    world
        .get_resource::<ShippedConfig>()
        .map_or(1.0, |ShippedConfig(config)| match bus {
            None => config.audio.master_volume,
            Some(bus) => config.audio.bus_volume(bus),
        })
}

#[cfg(test)]
#[path = "tests/settings.rs"]
mod tests;
