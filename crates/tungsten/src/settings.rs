//! The runtime apply of the player's [`Settings`] (`D-159`): what a settings
//! screen calls after changing them, before `Settings::save`.

use tungsten_core::{AudioCommands, Config, DisplayState, ScaleFactor, Settings, World};

use crate::app::resolve_startup_display;
use crate::display::{PendingDisplay, request_display_settings};

/// The config as `App::new` received it, before the settings: the baseline
/// every [`apply_settings`] starts from, so an unset value returns to the
/// shipped one.
pub(crate) struct ShippedConfig(pub(crate) Config);

/// Applies the world's [`Settings`] now, as a restart would: the display
/// mode, resolution and vsync through a display request, the UI scale to
/// [`ScaleFactor`], the master volume through [`AudioCommands`]. A value the
/// player unset returns to the shipped one, the UI scale and the master
/// volume to 1.0. A display request queued earlier this frame, such as a
/// hotkey's, takes the persisted values, so the last apply wins; the F9 and
/// F11 hotkeys' changes to them since startup are replaced. A function, not
/// a system: call it after changing `Settings`, then save them.
///
/// Without `App`'s shipped config (a `World` built by hand), unset display
/// values keep the current state. Without `Settings`, nothing happens.
pub fn apply_settings(world: &mut World) {
    let Some(settings) = world.get_resource::<Settings>().cloned() else {
        return;
    };
    if let Some(scale) = world.get_resource_mut::<ScaleFactor>() {
        scale.set_ui_scale(settings.ui_scale().unwrap_or(1.0));
    }
    if let Some(audio) = world.get_resource_mut::<AudioCommands>() {
        audio.set_master_volume(settings.master_volume().unwrap_or(1.0));
    }

    let current = world
        .get_resource::<DisplayState>()
        .copied()
        .unwrap_or_default();
    let pending = world
        .get_resource::<PendingDisplay>()
        .and_then(|pending| pending.0);
    let mut next = pending.unwrap_or(current);
    if let Some(ShippedConfig(shipped)) = world.get_resource::<ShippedConfig>() {
        let mut config = shipped.clone();
        if let Err(err) = config.apply_settings(&settings) {
            log::error!("Settings not applied: {err}");
            return;
        }
        let target = resolve_startup_display(&config);
        next.display_mode = target.display_mode;
        next.resolution = target.resolution;
        next.vsync = target.vsync;
        next.present_mode = target.present_mode;
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
    }

    if next == current {
        if let Some(pending) = world.get_resource_mut::<PendingDisplay>() {
            pending.0 = None;
        }
    } else if let Err(err) = request_display_settings(world, next) {
        log::error!("Display request rejected: {err}");
    }
}

#[cfg(test)]
#[path = "tests/settings.rs"]
mod tests;
