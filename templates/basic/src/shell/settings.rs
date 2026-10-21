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

use std::num::NonZeroU32;

use tungsten::core::settings::SettingsError;
use tungsten::core::ui::{Align, Sizing, SliderRange, UiEvent, UiTree, UiValue, WidgetId};
use tungsten::core::{
    AudioBus, DisplayMode, DisplayState, FrameRateCap, MonitorInfo, Resolution, Settings, World,
};
use tungsten::{GameState, StateContext, StateId, shipped_volume};

use super::confirm::{self, ConfirmState};
use super::keybindings::{self, KeybindingsState};
use super::options::{self, GameOptions};
use super::{
    Difficulty, RunState, Shell, apply_settings, enter, events, idle, pop, pressed, push, theme,
    toast, widgets,
};

pub const SETTINGS: StateId = "settings";
/// The tabs, in order.
pub const TABS: [&str; 5] = ["Display", "Audio", "Interface", "Controls", "Gameplay"];
/// The Gameplay tab.
pub const GAMEPLAY_TAB: usize = 4;
/// The window sizes offered without a monitor to read, smallest first; the
/// first is the least any monitor's list offers.
pub const SIZES: [(u32, u32); 4] = [(1280, 720), (1600, 900), (1920, 1080), (2560, 1440)];
/// The frame-rate limit's slider: its least, its most and its step, and
/// where it rests while Uncapped is on.
pub const FPS: [f32; 3] = [60.0, 480.0, 5.0];
pub const FPS_REST: u32 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Row {
    Mode,
    Size,
    Vsync,
    Cap,
    Uncapped,
    Bloom,
    Apply,
    Master,
    Music,
    Effects,
    MuteUnfocused,
    UiScale,
    TextScale,
    Keys,
    ResetKeys,
    Difficulty,
    Shake,
    Damage,
    PauseUnfocused,
    Autosave,
}

/// How a row is built: a button that cycles its value, a toggle, a slider
/// (least, most, step) or a plain button.
#[derive(Debug, Clone, Copy)]
enum Kind {
    Choice,
    Toggle,
    Slider(f32, f32, f32),
    Button,
}

const PERCENT: Kind = Kind::Slider(0.0, 100.0, 5.0);
const CAP: Kind = Kind::Slider(FPS[0], FPS[1], FPS[2]);
const TEXT: std::ops::RangeInclusive<f32> = Settings::TEXT_SCALE_RANGE;
/// The UI scales the shell offers: the engine's range to 2, since past it
/// the floor (`D-184`) lays most windows out no differently.
pub const UI_SCALES: std::ops::RangeInclusive<f32> = 0.5..=2.0;
const UI_SCALE: Kind = Kind::Slider(*UI_SCALES.start(), *UI_SCALES.end(), 0.25);
const TEXT_SCALE: Kind = Kind::Slider(*TEXT.start(), *TEXT.end(), 0.25);

/// Every row: its tab, its caption and how it is built.
const ROWS: [(Row, usize, &str, Kind); 20] = [
    (Row::Mode, 0, "Window mode", Kind::Choice),
    (Row::Size, 0, "Window size", Kind::Choice),
    (Row::Vsync, 0, "VSync", Kind::Toggle),
    (Row::Cap, 0, "Frame rate limit", CAP),
    (Row::Uncapped, 0, "Uncapped", Kind::Toggle),
    (Row::Bloom, 0, "Bloom", Kind::Toggle),
    (Row::Apply, 0, "Apply", Kind::Button),
    (Row::Master, 1, "Master", PERCENT),
    (Row::Music, 1, "Music", PERCENT),
    (Row::Effects, 1, "Effects", PERCENT),
    (Row::MuteUnfocused, 1, "Mute when unfocused", Kind::Toggle),
    (Row::UiScale, 2, "UI scale", UI_SCALE),
    (Row::TextScale, 2, "Text scale", TEXT_SCALE),
    (Row::Keys, 3, "Keybindings…", Kind::Button),
    (Row::ResetKeys, 3, "Reset all bindings", Kind::Button),
    (Row::Difficulty, 4, "Difficulty", Kind::Choice),
    (Row::Shake, 4, "Screen shake", PERCENT),
    (Row::Damage, 4, "Damage numbers", Kind::Toggle),
    (Row::PauseUnfocused, 4, "Pause when unfocused", Kind::Toggle),
    (Row::Autosave, 4, "Autosave", Kind::Toggle),
];

struct Ui {
    root: WidgetId,
    tabs: Vec<WidgetId>,
    bars: Vec<WidgetId>,
    panels: Vec<WidgetId>,
    /// Each row's control, in [`ROWS`]' order.
    rows: Vec<WidgetId>,
    /// Each slider's readout.
    readouts: Vec<(WidgetId, WidgetId)>,
    reset: WidgetId,
    back: WidgetId,
}

impl Ui {
    /// `row`'s control.
    fn control(&self, row: Row) -> WidgetId {
        let at = ROWS.iter().position(|(each, ..)| *each == row);
        self.rows[at.unwrap_or_default()]
    }
}

/// The settings screen, open on one tab.
pub struct SettingsState {
    tab: usize,
    ui: Option<Ui>,
    /// The scale slider the pointer holds: it applies at the release, so
    /// the UI is not rescaled under the pointer.
    held: Option<WidgetId>,
    /// The window mode and size picked and not yet applied.
    pending: (Option<DisplayMode>, Option<Resolution>),
}

impl SettingsState {
    #[must_use]
    pub fn new(tab: usize) -> Self {
        Self {
            tab,
            ui: None,
            held: None,
            pending: (None, None),
        }
    }
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

/// A slider's readout for `value`: a percent, the scales as percents of 1,
/// or frames a second.
fn readout(row: Row, value: f32) -> String {
    match row {
        Row::Cap => format!("{value:.0} fps"),
        Row::UiScale | Row::TextScale => format!("{:.0}%", value * 100.0),
        _ => format!("{value:.0}%"),
    }
}

/// The value after `value` in `values`, the first when it is none of them.
fn next<T: PartialEq + Copy>(values: &[T], value: T) -> T {
    let at = values.iter().position(|each| *each == value);
    values[at.map_or(0, |at| (at + 1) % values.len())]
}

impl SettingsState {
    fn select(&mut self, tree: &mut UiTree, tab: usize) {
        let Some(ui) = &self.ui else {
            return;
        };
        self.tab = tab;
        for (at, panel) in ui.panels.iter().enumerate() {
            widgets::show(tree, *panel, at == tab);
        }
        widgets::select_tab(tree, &ui.bars, tab);
        tree.focus(ui.tabs[tab]);
    }

    /// Writes every row's value from the world.
    fn refresh(&self, world: &mut World) {
        let Some(ui) = &self.ui else {
            return;
        };
        let (settings, display) = (world.resource::<Settings>().clone(), display(world));
        let options = *world.resource::<GameOptions>();
        let difficulty = world.get_resource::<RunState>().map(|run| run.difficulty);
        let volume = |bus: Option<AudioBus>| {
            let set = bus.map_or(settings.master_volume(), |bus| settings.bus_volume(bus));
            set.unwrap_or_else(|| shipped_volume(world, bus)) * 100.0
        };
        let volumes = [None, Some(AudioBus::Music), Some(AudioBus::Effects)].map(volume);
        let tree = world.resource_mut::<UiTree>();
        let (mode, size) = self.pending;
        let fullscreen = mode.unwrap_or(display.display_mode) != DisplayMode::Windowed;
        let cap = display.frame_rate_cap;
        for (&(row, ..), &id) in ROWS.iter().zip(&ui.rows) {
            let value = match row {
                Row::Mode => {
                    let mode = if fullscreen { "Fullscreen" } else { "Windowed" };
                    _ = tree.set_text(id, mode);
                    continue;
                }
                Row::Size => {
                    let Resolution { width, height } = size.unwrap_or(display.resolution);
                    let text = format!("{width}×{height} ({})", aspect(width, height));
                    tree.set_text(id, text.as_str());
                    tree.set_enabled(id, !fullscreen);
                    continue;
                }
                Row::Apply => {
                    tree.set_enabled(id, mode.is_some() || size.is_some());
                    continue;
                }
                Row::Cap => {
                    // A cap applies as stored: off the grid it reads exactly,
                    // under the floor it reads the floor the slider clamps it
                    // to; under Uncapped the slider keeps its value, off.
                    if let Some(fps) = cap {
                        tree.set_slider_value(id, fps as f32);
                    }
                    let held = tree.slider_value(id).unwrap_or(0.0);
                    let fps = cap.map_or(held, |fps| (fps as f32).max(FPS[0]));
                    tree.set_enabled(id, cap.is_some());
                    self.show(tree, id, readout(row, fps));
                    continue;
                }
                Row::Difficulty => {
                    let text = difficulty.map_or("Chosen at New Game", Difficulty::label);
                    tree.set_text(id, text);
                    tree.set_enabled(id, difficulty.is_some());
                    continue;
                }
                Row::Keys | Row::ResetKeys => continue,
                Row::Vsync => UiValue::Bool(display.vsync),
                Row::Uncapped => UiValue::Bool(cap.is_none()),
                Row::Bloom => UiValue::Bool(options.bloom),
                Row::MuteUnfocused => UiValue::Bool(options.mute_unfocused),
                Row::Damage => UiValue::Bool(options.damage_numbers),
                Row::PauseUnfocused => UiValue::Bool(options.pause_unfocused),
                Row::Autosave => UiValue::Bool(options.autosave),
                Row::Master => UiValue::Number(volumes[0]),
                Row::Music => UiValue::Number(volumes[1]),
                Row::Effects => UiValue::Number(volumes[2]),
                Row::UiScale => UiValue::Number(settings.ui_scale().unwrap_or(theme::UI_SCALE)),
                Row::TextScale => UiValue::Number(settings.text_scale().unwrap_or(1.0)),
                Row::Shake => UiValue::Number(options.screen_shake() * 100.0),
            };
            match value {
                UiValue::Bool(on) => _ = tree.set_toggled(id, on),
                _ => {
                    let number = value.as_number().unwrap_or(0.0);
                    tree.set_slider_value(id, number);
                    self.show(tree, id, readout(row, number));
                }
            }
        }
    }

    /// Writes `text` into `slider`'s readout.
    fn show(&self, tree: &mut UiTree, slider: WidgetId, text: String) {
        let readouts = self.ui.iter().flat_map(|ui| &ui.readouts);
        if let Some((_, label)) = readouts.into_iter().find(|(id, _)| *id == slider) {
            tree.set_plain_text(*label, &text);
        }
    }

    /// A row's button activated: the window mode and size are picked, the
    /// value as it runs dropping the pick, and Apply sends the picks.
    fn activate(&mut self, world: &mut World, row: Row) {
        let display = display(world);
        let (mode, size) = &mut self.pending;
        match row {
            Row::Mode => {
                let mode_now = mode.unwrap_or(display.display_mode);
                let picked = match mode_now {
                    DisplayMode::Windowed => DisplayMode::BorderlessFullscreen,
                    _ => DisplayMode::Windowed,
                };
                *mode = (picked != display.display_mode).then_some(picked);
            }
            Row::Size => {
                let picked = next(&window_sizes(world), size.unwrap_or(display.resolution));
                *size = (picked != display.resolution).then_some(picked);
            }
            Row::Apply => {
                let (mode, size) = std::mem::take(&mut self.pending);
                if let Some(ui) = &self.ui {
                    let first = ui.control(Row::Mode);
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
            Row::Difficulty => {
                if let Some(run) = world.get_resource_mut::<RunState>() {
                    run.difficulty = next(&Difficulty::ALL, run.difficulty);
                }
            }
            Row::Keys => push(world, KeybindingsState::default()),
            Row::ResetKeys => {
                keybindings::reset(world, None);
                toast::toast(world, "Bindings reset");
            }
            _ => {}
        }
    }

    /// A slider or toggle took `value`: the frame-rate limit the slider's,
    /// or none while Uncapped is on.
    fn set(&self, world: &mut World, row: Row, value: UiValue) {
        let number = value.as_number().unwrap_or(0.0);
        let on = value.as_bool().unwrap_or(false);
        let fps = |fps: f32| NonZeroU32::new(fps as u32).map(FrameRateCap::Fps);
        match row {
            Row::Vsync => change(world, |settings| settings.set_vsync(Some(on))),
            Row::Cap => change(world, |settings| settings.set_frame_rate_cap(fps(number))),
            Row::Uncapped => {
                let slider = self.ui.as_ref().map(|ui| ui.control(Row::Cap));
                let value = slider.and_then(|id| world.resource::<UiTree>().slider_value(id));
                let cap = if on {
                    Some(FrameRateCap::Uncapped)
                } else {
                    value.and_then(fps)
                };
                change(world, |settings| settings.set_frame_rate_cap(cap));
            }
            Row::Master => set_volume(world, None, number),
            Row::Music => set_volume(world, Some(AudioBus::Music), number),
            Row::Effects => set_volume(world, Some(AudioBus::Effects), number),
            Row::UiScale | Row::TextScale => set_scale(world, row == Row::TextScale, number),
            _ => {
                let options = world.resource_mut::<GameOptions>();
                match row {
                    Row::MuteUnfocused => options.mute_unfocused = on,
                    Row::Damage => options.damage_numbers = on,
                    Row::PauseUnfocused => options.pause_unfocused = on,
                    Row::Autosave => options.autosave = on,
                    Row::Bloom => options.bloom = on,
                    Row::Shake => options.set_screen_shake(number / 100.0),
                    _ => {}
                }
                options::apply(world);
            }
        }
    }

    /// Returns the open tab's values to unset or default, dropping the
    /// display's picks.
    fn reset(&mut self, world: &mut World) {
        if self.tab == 0 {
            self.pending = (None, None);
        }
        let defaults = GameOptions::default();
        change(world, |settings| match self.tab {
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
        match self.tab {
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
        self.refresh(world);
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
        let tree = ctx.world.resource_mut::<UiTree>();
        let (root, card) = widgets::screen(tree, theme::WIDE_WIDTH);
        widgets::heading(tree, card, "Settings", false);
        let body = widgets::row(tree, card);
        let mut style = tree.layout_style(body).cloned().unwrap_or_default();
        style.align_items = Align::Start;
        tree.set_layout_style(body, style);
        let (tabs, bars) = widgets::tab_rail(tree, body, &TABS);
        let panels: Vec<WidgetId> = TABS.iter().map(|_| widgets::column(tree, body)).collect();
        for panel in &panels {
            widgets::set_width(tree, *panel, Sizing::Fill);
        }
        let mut readouts = Vec::new();
        let rows = ROWS
            .iter()
            .map(|&(row, tab, caption, kind)| match kind {
                Kind::Choice => widgets::choice_row(tree, panels[tab], caption, ""),
                Kind::Toggle => widgets::toggle_row(tree, panels[tab], caption, false),
                Kind::Button => {
                    let button = widgets::button(tree, panels[tab], caption);
                    // Apply sits small at the tab's foot, under the values.
                    if row == Row::Apply {
                        let mut style = tree.layout_style(button).cloned().unwrap_or_default();
                        style.align_self = Some(Align::End);
                        tree.set_layout_style(button, style);
                    }
                    button
                }
                Kind::Slider(least, most, step) => {
                    let range = SliderRange::new(least, most).with_step(step);
                    let row = widgets::slider_row(tree, panels[tab], caption, range);
                    readouts.push(row);
                    row.0
                }
            })
            .collect();
        let actions = widgets::row(tree, card);
        let reset = widgets::button(tree, actions, "Reset tab");
        let back = widgets::button(tree, actions, "Back");
        let ui = Ui {
            root,
            tabs,
            bars,
            panels,
            rows,
            readouts,
            reset,
            back,
        };
        tree.set_slider_value(ui.control(Row::Cap), FPS_REST as f32);
        self.ui = Some(ui);
        self.select(tree, self.tab);
        enter(ctx.world, root);
        self.refresh(ctx.world);
        ctx.world.resource_mut::<Shell>().settings_open = true;
    }

    fn on_exit(&mut self, ctx: &mut StateContext) {
        ctx.world.resource_mut::<Shell>().settings_open = false;
    }

    fn on_resume(&mut self, ctx: &mut StateContext) {
        self.refresh(ctx.world);
    }

    fn update(&mut self, world: &mut World) {
        let Some(ui) = &self.ui else {
            return;
        };
        if !idle(world) {
            return;
        }
        let (root, reset, back, tabs, rows) =
            (ui.root, ui.reset, ui.back, ui.tabs.clone(), ui.rows.clone());
        let step = match ["ui_next", "ui_prev"].map(|key| pressed(world, key, false)) {
            [true, false] => 1,
            [false, true] => TABS.len() - 1,
            _ => 0,
        };
        if step > 0 {
            let tab = (self.tab + step) % TABS.len();
            self.select(world.resource_mut::<UiTree>(), tab);
        }
        let (events, mut stale) = (events(world), false);
        let row_of = |widget: WidgetId| {
            rows.iter()
                .position(|id| *id == widget)
                .map(|at| ROWS[at].0)
        };
        for event in events {
            match event {
                UiEvent::Activated { widget, .. } if widget == back => return Self::leave(world),
                UiEvent::Cancel { scope, .. } if scope == root => return Self::leave(world),
                UiEvent::Activated { widget, .. } if widget == reset => self.reset(world),
                UiEvent::Activated { widget, .. } if tabs.contains(&widget) => {
                    let tab = tabs.iter().position(|tab| *tab == widget).unwrap_or(0);
                    self.select(world.resource_mut::<UiTree>(), tab);
                }
                // The rows refresh after the loop, once this routing's
                // `ValueChanged` has applied a toggle's new value.
                UiEvent::Activated { widget, .. } => {
                    if let Some(row) = row_of(widget) {
                        self.activate(world, row);
                        stale = true;
                    }
                }
                UiEvent::Pressed { widget, .. }
                    if matches!(row_of(widget), Some(Row::UiScale | Row::TextScale)) =>
                {
                    self.held = Some(widget);
                }
                UiEvent::Released { widget, .. } | UiEvent::PressCancelled { widget, .. }
                    if self.held == Some(widget) =>
                {
                    self.held = None;
                    let value = world
                        .resource::<UiTree>()
                        .slider_value(widget)
                        .unwrap_or(1.0);
                    let row = row_of(widget).unwrap_or(Row::UiScale);
                    self.set(world, row, UiValue::Number(value));
                }
                UiEvent::ValueChanged { widget, value, .. } => {
                    let Some(row) = row_of(widget) else {
                        continue;
                    };
                    if let Some(number) = value.as_number() {
                        // A held scale slider shows its value now and
                        // applies it at the release.
                        self.show(world.resource_mut::<UiTree>(), widget, readout(row, number));
                    }
                    if self.held != Some(widget) {
                        self.set(world, row, value);
                    }
                }
                _ => {}
            }
        }
        if stale {
            self.refresh(world);
        }
    }
}
