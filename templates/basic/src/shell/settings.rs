//! Tabbed settings: Display, Audio, Interface, Controls and Gameplay. Every
//! change applies at once through `shell::apply_settings` and is written
//! when the screen closes (Back or `ui_cancel` save `Settings` and the
//! gameplay options) or the game exits with it open; a tab's Reset returns
//! its values to unset or default.
//! The window mode and size are picked first and sent by Apply, so a player
//! cycles sizes freely; Apply then asks to keep them, reverting after 10 s.
//! The tabs sit in a rail beside the rows; `ui_next` and `ui_prev` step
//! through them. Every slider shows its value in a mono readout. The
//! Display rows show the set value, else the live one, and the shell keeps
//! the F11 and F9 keys' changes set; the volumes show the set value, else
//! the one `tungsten.json` ships.
//! The rows, their kinds, ranges and keys are `assets/ui/screens/settings.json`;
//! their values are the bindings [`bind`] registers over a [`SettingsView`],
//! and each control is handled by its key.

use std::num::NonZeroU32;

use tungsten::core::settings::SettingsError;
use tungsten::core::ui::{UiBindings, UiEvent, UiKey, UiTree, UiValue, WidgetId, screen_build};
use tungsten::core::{
    AudioBus, DisplayMode, DisplayState, FrameRateCap, MonitorInfo, Resolution, Settings, World,
};
use tungsten::{GameState, StateContext, StateId, shipped_volume};

use super::confirm::{self, ConfirmState};
use super::keybindings::{self, KeybindingsState};
use super::options::{self, GameOptions};
use super::{
    BACK, Difficulty, Menu, RunState, Shell, apply_settings, enter, events, idle, pop, pressed,
    push, stack, theme, toast,
};

pub const SETTINGS: StateId = "settings";
/// The tabs, in order, and their keys.
pub const TABS: [&str; 5] = ["Display", "Audio", "Interface", "Controls", "Gameplay"];
const TAB_KEYS: [&str; 5] = ["display", "audio", "interface", "controls", "gameplay"];
const RESET_TAB: UiKey = UiKey::new("reset_tab");
/// The Gameplay tab.
pub const GAMEPLAY_TAB: usize = 4;
/// The window sizes offered without a monitor to read, smallest first; the
/// first is the least any monitor's list offers.
pub const SIZES: [(u32, u32); 4] = [(1280, 720), (1600, 900), (1920, 1080), (2560, 1440)];
/// The frame-rate limit's slider: its least, its most and its step, and
/// where it rests while Uncapped is on.
pub const FPS: [f32; 3] = [60.0, 480.0, 5.0];
pub const FPS_REST: u32 = 60;
/// The UI scales the shell offers: the engine's range to 2, since past it
/// the floor (`D-184`) lays most windows out no differently.
pub const UI_SCALES: std::ops::RangeInclusive<f32> = 0.5..=2.0;

/// What the screen shows beyond `Settings` and the options: the open tab,
/// the window mode and size picked and not yet applied, where the frame
/// cap's slider rests while Uncapped is on, and the scale slider the
/// pointer holds with its value, applied at the release so the UI is not
/// rescaled under the pointer. A resource [`SettingsState`] owns; absent,
/// the Display tab with nothing pending.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SettingsView {
    tab: usize,
    pending: (Option<DisplayMode>, Option<Resolution>),
    rest: f32,
    held: Option<(WidgetId, f32)>,
}

impl Default for SettingsView {
    fn default() -> Self {
        let (tab, pending, rest, held) = (0, (None, None), FPS_REST as f32, None);
        Self {
            tab,
            pending,
            rest,
            held,
        }
    }
}

fn view(world: &World) -> SettingsView {
    world
        .get_resource::<SettingsView>()
        .copied()
        .unwrap_or_default()
}

fn view_mut(world: &mut World) -> &mut SettingsView {
    world.resource_mut::<SettingsView>()
}

/// Changes `Settings` with `edit` and applies them.
fn change(world: &mut World, edit: impl FnOnce(&mut Settings)) {
    edit(world.resource_mut::<Settings>());
    apply_settings(world);
}

/// Sets the master volume (`bus` `None`) or a bus's to `percent`, which
/// `Settings` clamps to 0..=100 %, ignoring a value that is not finite.
pub fn set_volume(world: &mut World, bus: Option<AudioBus>, percent: f32) {
    change(world, |settings| match bus {
        Some(bus) => settings.set_bus_volume(bus, Some(percent / 100.0)),
        None => settings.set_master_volume(Some(percent / 100.0)),
    });
}

/// Sets the UI scale, clamped to [`UI_SCALES`], or with `text` the text
/// scale, which `Settings` clamps to its range; a value that is not finite
/// is ignored.
pub fn set_scale(world: &mut World, text: bool, scale: f32) {
    change(world, |settings| match text {
        true => settings.set_text_scale(Some(scale)),
        false if scale.is_finite() => {
            let (least, most) = (*UI_SCALES.start(), *UI_SCALES.end());
            settings.set_ui_scale(Some(scale.clamp(least, most)));
        }
        false => {}
    });
}

/// The window mode and size Apply sends: applied, then kept only when the
/// player says so within 10 s.
fn change_display(world: &mut World, edit: impl FnOnce(&mut Settings)) {
    let before = world.resource::<Settings>().clone();
    change(world, edit);
    let (mode, size) = (before.display_mode(), before.resolution());
    let keep = ConfirmState::new("Keep these display settings?", "Keep", "Revert");
    confirm::ask(
        world,
        keep.countdown(10.0).on_no(move |world| {
            change(world, |settings| {
                settings.set_display_mode(mode);
                settings.set_resolution(size);
            });
        }),
    );
}

/// The display as set, else as it runs.
fn display(world: &World) -> DisplayState {
    let settings = world.resource::<Settings>();
    let mut live = world
        .get_resource::<DisplayState>()
        .copied()
        .unwrap_or_default();
    live.display_mode = settings.display_mode().unwrap_or(live.display_mode);
    live.resolution = settings.resolution().unwrap_or(live.resolution);
    live.vsync = settings.vsync().unwrap_or(live.vsync);
    live.frame_rate_cap = match settings.frame_rate_cap() {
        Some(FrameRateCap::Fps(fps)) => Some(fps.get()),
        Some(FrameRateCap::Uncapped) => None,
        None => live.frame_rate_cap,
    };
    live
}

/// The window sizes offered, smallest first: the monitor's modes from the
/// first of [`SIZES`] up to its current size, with [`SIZES`] that fit under
/// a monitor listing one mode, and [`SIZES`] alone without a monitor.
#[must_use]
pub fn window_sizes(world: &World) -> Vec<Resolution> {
    let fixed = SIZES.map(|(width, height)| Resolution { width, height });
    let Some(monitor) = world.get_resource::<MonitorInfo>() else {
        return fixed.to_vec();
    };
    let desktop = monitor.size;
    let fits = |size: &Resolution| size.width <= desktop.width && size.height <= desktop.height;
    let mut sizes = monitor.window_sizes();
    if monitor.modes.len() <= 1 {
        sizes.extend(fixed.iter().filter(|size| fits(size)));
        sizes.sort_by_key(|size| (u64::from(size.width) * u64::from(size.height), size.width));
        sizes.dedup();
    }
    sizes
        .retain(|size| (size.width >= SIZES[0].0 && size.height >= SIZES[0].1) || *size == desktop);
    sizes
}

/// A size's aspect ratio as displays are sold by it: the nearest common
/// one within 3 %, else the size reduced.
#[must_use]
pub fn aspect(width: u32, height: u32) -> String {
    const COMMON: [(u32, u32); 7] = [(16, 9), (16, 10), (4, 3), (5, 4), (3, 2), (21, 9), (32, 9)];
    let ratio = width as f32 / height.max(1) as f32;
    let near = COMMON
        .iter()
        .find(|(w, h)| (ratio * *h as f32 / *w as f32 - 1.0).abs() < 0.03);
    let gcd = |mut a: u32, mut b: u32| {
        while b != 0 {
            (a, b) = (b, a % b);
        }
        a.max(1)
    };
    let (w, h) = near.copied().unwrap_or_else(|| {
        let d = gcd(width, height);
        (width / d, height / d)
    });
    format!("{w}:{h}")
}

/// The value after `value` in `values`, the first when it is none of them.
fn next<T: PartialEq + Copy>(values: &[T], value: T) -> T {
    let at = values.iter().position(|each| *each == value);
    values[at.map_or(0, |at| (at + 1) % values.len())]
}

/// Whether the window is fullscreen as picked, else as set or running.
fn fullscreen(world: &World) -> bool {
    let mode = view(world).pending.0;
    mode.unwrap_or(display(world).display_mode) != DisplayMode::Windowed
}

/// The master volume (`bus` `None`) or a bus's, in percent: as set, else
/// as `tungsten.json` ships it.
fn volume(world: &World, bus: Option<AudioBus>) -> f32 {
    let settings = world.resource::<Settings>();
    let set = bus.map_or(settings.master_volume(), |bus| settings.bus_volume(bus));
    set.unwrap_or_else(|| shipped_volume(world, bus)) * 100.0
}

/// A scale as set, else its default, or the held slider's value while the
/// pointer holds the slider keyed `key`.
fn scale(world: &World, key: &str) -> f32 {
    let held = view(world)
        .held
        .filter(|(slider, _)| world.resource::<UiTree>().key_name(*slider) == Some(key));
    let settings = world.resource::<Settings>();
    let set = match key {
        "ui_scale" => settings.ui_scale().unwrap_or(theme::UI_SCALE),
        _ => settings.text_scale().unwrap_or(1.0),
    };
    held.map_or(set, |(_, value)| value)
}

/// The frame cap's slider: the cap as set or running, else where it rests.
fn cap(world: &World) -> f32 {
    display(world)
        .frame_rate_cap
        .map_or(view(world).rest, |fps| fps as f32)
}

fn options(world: &World) -> GameOptions {
    *world.resource::<GameOptions>()
}

fn on(value: bool) -> UiValue {
    UiValue::Bool(value)
}

fn number(value: f32) -> UiValue {
    UiValue::Number(value)
}

/// The settings screen's bindings, which `assets/ui/screens/settings.json`
/// names: each row's value as set, else as it runs, its readout, which rows
/// are enabled, and which tab shows.
pub(super) fn bind(bindings: &mut UiBindings) {
    bindings
        .text("window_mode", |w| {
            ["Windowed", "Fullscreen"][usize::from(fullscreen(w))].to_owned()
        })
        .text("window_size", |w| {
            let size = view(w).pending.1.unwrap_or(display(w).resolution);
            let (width, height) = (size.width, size.height);
            format!("{width}×{height} ({})", aspect(width, height))
        })
        .enabled("windowed", |w| !fullscreen(w))
        .value("vsync", |w| on(display(w).vsync))
        .value("frame_rate_limit", |w| number(cap(w)))
        .text("frame_rate_text", |w| {
            let floor = |fps: u32| (fps as f32).max(FPS[0]);
            let fps = display(w).frame_rate_cap.map_or(view(w).rest, floor);
            format!("{fps:.0} fps")
        })
        .enabled("capped", |w| display(w).frame_rate_cap.is_some())
        .value("uncapped", |w| on(display(w).frame_rate_cap.is_none()))
        .value("bloom", |w| on(options(w).bloom))
        .enabled("pending", |w| view(w).pending != (None, None))
        .value("master", |w| number(volume(w, None)))
        .value("music", |w| number(volume(w, Some(AudioBus::Music))))
        .value("effects", |w| number(volume(w, Some(AudioBus::Effects))))
        .text("master_text", |w| format!("{:.0}%", volume(w, None)))
        .text("music_text", |w| {
            format!("{:.0}%", volume(w, Some(AudioBus::Music)))
        })
        .text("effects_text", |w| {
            format!("{:.0}%", volume(w, Some(AudioBus::Effects)))
        })
        .value("mute_unfocused", |w| on(options(w).mute_unfocused))
        .value("ui_scale", |w| {
            number(
                w.resource::<Settings>()
                    .ui_scale()
                    .unwrap_or(theme::UI_SCALE),
            )
        })
        .value("text_scale", |w| {
            number(w.resource::<Settings>().text_scale().unwrap_or(1.0))
        })
        .text("ui_scale_text", |w| {
            format!("{:.0}%", scale(w, "ui_scale") * 100.0)
        })
        .text("text_scale_text", |w| {
            format!("{:.0}%", scale(w, "text_scale") * 100.0)
        })
        .text("difficulty", |w| {
            let run = w.get_resource::<RunState>();
            run.map_or("Chosen at New Game", |run| run.difficulty.label())
                .to_owned()
        })
        .enabled("in_run", |w| w.has_resource::<RunState>())
        .value("screen_shake", |w| {
            number(options(w).screen_shake() * 100.0)
        })
        .text("screen_shake_text", |w| {
            format!("{:.0}%", options(w).screen_shake() * 100.0)
        })
        .value("damage_numbers", |w| on(options(w).damage_numbers))
        .value("pause_unfocused", |w| on(options(w).pause_unfocused))
        .value("autosave", |w| on(options(w).autosave))
        .value("settings_tab", |w| number(view(w).tab as f32))
        .visible("display_tab", |w| view(w).tab == 0)
        .visible("audio_tab", |w| view(w).tab == 1)
        .visible("interface_tab", |w| view(w).tab == 2)
        .visible("controls_tab", |w| view(w).tab == 3)
        .visible("gameplay_tab", |w| view(w).tab == GAMEPLAY_TAB);
}

/// The settings screen, open on one tab.
pub struct SettingsState {
    tab: usize,
    root: Option<WidgetId>,
}

impl SettingsState {
    #[must_use]
    pub fn new(tab: usize) -> Self {
        Self { tab, root: None }
    }

    /// Shows `tab` and focuses its button.
    fn select(&self, world: &mut World, tab: usize) {
        let Some(root) = self.root else {
            return;
        };
        view_mut(world).tab = tab;
        screen_build::refresh(world, root);
        if let Some(button) = screen_build::node(world, root, TAB_KEYS[tab]) {
            world.resource_mut::<UiTree>().focus(button);
        }
    }

    /// The control keyed `key`'s button activated: the window mode and size
    /// are picked, the value as it runs dropping the pick, and Apply sends
    /// the picks.
    fn activate(&self, world: &mut World, key: &str) {
        let (display, (mode, size)) = (display(world), view(world).pending);
        match key {
            "window_mode" => {
                let picked = match mode.unwrap_or(display.display_mode) {
                    DisplayMode::Windowed => DisplayMode::BorderlessFullscreen,
                    _ => DisplayMode::Windowed,
                };
                view_mut(world).pending.0 = (picked != display.display_mode).then_some(picked);
            }
            "window_size" => {
                let picked = next(&window_sizes(world), size.unwrap_or(display.resolution));
                view_mut(world).pending.1 = (picked != display.resolution).then_some(picked);
            }
            "apply" => {
                let (mode, size) = std::mem::take(&mut view_mut(world).pending);
                let first = self
                    .root
                    .and_then(|root| screen_build::node(world, root, "window_mode"));
                if let Some(first) = first {
                    world.resource_mut::<UiTree>().focus(first);
                }
                change_display(world, |settings| {
                    if mode.is_some() {
                        settings.set_display_mode(mode);
                    }
                    if size.is_some() {
                        settings.set_resolution(size);
                    }
                });
            }
            "difficulty" => {
                if let Some(run) = world.get_resource_mut::<RunState>() {
                    run.difficulty = next(&Difficulty::ALL, run.difficulty);
                }
            }
            "keybindings" => push(world, KeybindingsState::default()),
            "reset_bindings" => {
                keybindings::reset(world, None);
                toast::toast(world, "Bindings reset");
            }
            _ => {}
        }
    }

    /// The slider or toggle keyed `key` took `value`: the frame-rate limit
    /// the slider's, or none while Uncapped is on, where the slider rests.
    fn set(&self, world: &mut World, key: &str, value: UiValue) {
        let number = value.as_number().unwrap_or(0.0);
        let on = value.as_bool().unwrap_or(false);
        let fps = |fps: f32| NonZeroU32::new(fps as u32).map(FrameRateCap::Fps);
        match key {
            "vsync" => change(world, |settings| settings.set_vsync(Some(on))),
            "frame_rate_limit" => {
                change(world, |settings| settings.set_frame_rate_cap(fps(number)))
            }
            "uncapped" => {
                let slider = self
                    .root
                    .and_then(|root| screen_build::node(world, root, "frame_rate_limit"));
                let value = slider.and_then(|id| world.resource::<UiTree>().slider_value(id));
                let cap = if on {
                    view_mut(world).rest = value.unwrap_or(FPS_REST as f32);
                    Some(FrameRateCap::Uncapped)
                } else {
                    value.and_then(fps)
                };
                change(world, |settings| settings.set_frame_rate_cap(cap));
            }
            "master" => set_volume(world, None, number),
            "music" => set_volume(world, Some(AudioBus::Music), number),
            "effects" => set_volume(world, Some(AudioBus::Effects), number),
            "ui_scale" | "text_scale" => set_scale(world, key == "text_scale", number),
            "mute_unfocused" | "damage_numbers" | "pause_unfocused" | "autosave" | "bloom"
            | "screen_shake" => {
                let options = world.resource_mut::<GameOptions>();
                match key {
                    "mute_unfocused" => options.mute_unfocused = on,
                    "damage_numbers" => options.damage_numbers = on,
                    "pause_unfocused" => options.pause_unfocused = on,
                    "autosave" => options.autosave = on,
                    "bloom" => options.bloom = on,
                    _ => options.set_screen_shake(number / 100.0),
                }
                options::apply(world);
            }
            _ => {}
        }
    }

    /// Returns the open tab's values to unset or default, dropping the
    /// display's picks.
    fn reset(&self, world: &mut World) {
        let tab = view(world).tab;
        if tab == 0 {
            view_mut(world).pending = (None, None);
        }
        let defaults = GameOptions::default();
        change(world, |settings| match tab {
            0 => {
                settings.set_display_mode(None);
                settings.set_resolution(None);
                settings.set_vsync(None);
                settings.set_frame_rate_cap(None);
            }
            1 => {
                settings.set_master_volume(None);
                AudioBus::ALL
                    .iter()
                    .for_each(|bus| settings.set_bus_volume(*bus, None));
            }
            2 => {
                settings.set_ui_scale(None);
                settings.set_text_scale(None);
            }
            _ => {}
        });
        // Each tab's own options only: Display's Bloom, Audio's mute, and
        // Gameplay's rest.
        let options = world.resource_mut::<GameOptions>();
        let (mute_unfocused, bloom) = (options.mute_unfocused, options.bloom);
        match tab {
            0 => options.bloom = defaults.bloom,
            1 => options.mute_unfocused = defaults.mute_unfocused,
            3 => keybindings::reset(world, None),
            GAMEPLAY_TAB => {
                *options = defaults;
                (options.mute_unfocused, options.bloom) = (mute_unfocused, bloom);
                options::apply(world);
            }
            _ => {}
        }
    }

    /// Writes `Settings` and the options, then closes.
    fn leave(world: &mut World) {
        save(world);
        pop(world);
    }
}

/// Writes `Settings` and the gameplay options; a failure logs, and no user
/// folder saves nothing. The exit's last frame calls it while the screen is
/// open, and the F11 and F9 keys while it is not.
pub(crate) fn save(world: &World) {
    match world.resource::<Settings>().save() {
        Ok(()) | Err(SettingsError::NoFolder) => {}
        Err(error) => log::warn!("Settings not saved: {error}"),
    }
    options::save(world);
}

impl GameState for SettingsState {
    fn id(&self) -> StateId {
        SETTINGS
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        let world = &mut *ctx.world;
        let tab = self.tab;
        world.insert_resource(SettingsView {
            tab,
            ..SettingsView::default()
        });
        stack(world).own_resource::<SettingsView>();
        let root = Menu::build(world, "settings").root;
        self.root = Some(root);
        self.select(world, tab);
        enter(world, root);
        world.resource_mut::<Shell>().settings_open = true;
    }

    fn on_exit(&mut self, ctx: &mut StateContext) {
        ctx.world.resource_mut::<Shell>().settings_open = false;
    }

    fn on_resume(&mut self, ctx: &mut StateContext) {
        if let Some(root) = self.root {
            screen_build::refresh(ctx.world, root);
        }
    }

    fn update(&mut self, world: &mut World) {
        let Some(root) = self.root else {
            return;
        };
        if idle(world) && self.routed(world, root) {
            return;
        }
        screen_build::refresh(world, root);
    }
}

impl SettingsState {
    /// This routing's events and tab keys; `true` when the screen closed.
    fn routed(&mut self, world: &mut World, root: WidgetId) -> bool {
        let step = match ["ui_next", "ui_prev"].map(|key| pressed(world, key, false)) {
            [true, false] => 1,
            [false, true] => TABS.len() - 1,
            _ => 0,
        };
        if step > 0 {
            let tab = (view(world).tab + step) % TABS.len();
            self.select(world, tab);
        }
        let tab_of = |key: UiKey| TAB_KEYS.iter().position(|tab| UiKey::new(tab) == key);
        let key = |world: &World, widget| {
            let tree = world.resource::<UiTree>();
            tree.key_name(widget).unwrap_or_default().to_owned()
        };
        let scale = |name: &str| matches!(name, "ui_scale" | "text_scale");
        for event in events(world) {
            match event {
                UiEvent::Activated {
                    key: Some(BACK), ..
                } => {
                    Self::leave(world);
                    return true;
                }
                UiEvent::Cancel { scope, .. } if scope == root => {
                    Self::leave(world);
                    return true;
                }
                UiEvent::Activated {
                    key: Some(RESET_TAB),
                    ..
                } => self.reset(world),
                UiEvent::Activated {
                    widget, key: id, ..
                } => match id.and_then(tab_of) {
                    Some(tab) => self.select(world, tab),
                    None => self.activate(world, &key(world, widget)),
                },
                UiEvent::Pressed { widget, .. } if scale(&key(world, widget)) => {
                    let value = world.resource::<UiTree>().slider_value(widget);
                    view_mut(world).held = Some((widget, value.unwrap_or(1.0)));
                }
                UiEvent::Released { widget, .. } | UiEvent::PressCancelled { widget, .. }
                    if view(world).held.is_some_and(|(held, _)| held == widget) =>
                {
                    view_mut(world).held = None;
                    let value = world.resource::<UiTree>().slider_value(widget);
                    let number = UiValue::Number(value.unwrap_or(1.0));
                    self.set(world, &key(world, widget), number);
                }
                UiEvent::ValueChanged { widget, value, .. } => {
                    // A held scale slider shows its value now and applies
                    // it at the release.
                    match view(world).held {
                        Some((slider, draft)) if slider == widget => {
                            let draft = value.as_number().unwrap_or(draft);
                            view_mut(world).held = Some((slider, draft));
                        }
                        _ => self.set(world, &key(world, widget), value),
                    }
                }
                _ => {}
            }
        }
        false
    }
}
