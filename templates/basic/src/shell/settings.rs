//! Tabbed settings: Display, Audio, Interface, Controls and Gameplay. Every
//! change applies at once through `shell::apply_settings` and is written
//! when the screen closes (Back or `ui_cancel` save `Settings` and the
//! gameplay options); a tab's Reset returns its values to unset or default.
//! A window mode or size change asks to keep it, reverting after 10 s.
//! `ui_next` and `ui_prev` step through the tabs. The Display rows show the
//! set value, else the live one, so an F11 press shows while it is unset.

use std::num::NonZeroU32;

use tungsten::core::ui::{SliderRange, UiEvent, UiTree, UiValue, WidgetId};
use tungsten::core::{
    AudioBus, DisplayMode, DisplayState, FrameRateCap, Resolution, Settings, World,
};
use tungsten::{GameState, RootCover, StateContext, StateId};

use super::confirm::{self, ConfirmState};
use super::keybindings::{self, KeybindingsState};
use super::options::{self, GameOptions};
use super::{
    Difficulty, RunState, apply_settings, events, idle, pop, pressed, push, stack, theme, toast,
    widgets,
};

pub const SETTINGS: StateId = "settings";
/// The tabs, in order.
pub const TABS: [&str; 5] = ["Display", "Audio", "Interface", "Controls", "Gameplay"];
/// The Gameplay tab, which pause's Gameplay Options opens.
pub const GAMEPLAY_TAB: usize = 4;
/// The window sizes offered, smallest first.
pub const SIZES: [(u32, u32); 4] = [(1280, 720), (1600, 900), (1920, 1080), (2560, 1440)];
/// The frame-rate limits offered; 0 is none.
pub const CAPS: [u32; 5] = [0, 30, 60, 120, 144];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Row {
    Mode,
    Size,
    Vsync,
    Cap,
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
const UI: std::ops::RangeInclusive<f32> = Settings::UI_SCALE_RANGE;
const TEXT: std::ops::RangeInclusive<f32> = Settings::TEXT_SCALE_RANGE;

/// Every row: its tab, its caption and how it is built.
const ROWS: [(Row, usize, &str, Kind); 16] = [
    (Row::Mode, 0, "Window mode", Kind::Choice),
    (Row::Size, 0, "Window size", Kind::Choice),
    (Row::Vsync, 0, "VSync", Kind::Toggle),
    (Row::Cap, 0, "Frame rate limit", Kind::Choice),
    (Row::Master, 1, "Master", PERCENT),
    (Row::Music, 1, "Music", PERCENT),
    (Row::Effects, 1, "Effects", PERCENT),
    (Row::MuteUnfocused, 1, "Mute when unfocused", Kind::Toggle),
    (
        Row::UiScale,
        2,
        "UI scale",
        Kind::Slider(*UI.start(), *UI.end(), 0.25),
    ),
    (
        Row::TextScale,
        2,
        "Text scale",
        Kind::Slider(*TEXT.start(), *TEXT.end(), 0.25),
    ),
    (Row::Keys, 3, "Keybindings…", Kind::Button),
    (Row::ResetKeys, 3, "Reset all bindings", Kind::Button),
    (Row::Difficulty, 4, "Difficulty", Kind::Choice),
    (Row::Shake, 4, "Screen shake", PERCENT),
    (Row::Damage, 4, "Damage numbers", Kind::Toggle),
    (Row::PauseUnfocused, 4, "Pause when unfocused", Kind::Toggle),
];

struct Ui {
    root: WidgetId,
    tabs: Vec<WidgetId>,
    bars: Vec<WidgetId>,
    panels: Vec<WidgetId>,
    /// Each row's control, in [`ROWS`]' order.
    rows: Vec<WidgetId>,
    reset: WidgetId,
    back: WidgetId,
}

/// The settings screen, open on one tab.
pub struct SettingsState {
    tab: usize,
    ui: Option<Ui>,
    /// The scale slider the pointer holds: it applies at the release, so
    /// the UI is not rescaled under the pointer.
    held: Option<WidgetId>,
}

impl SettingsState {
    #[must_use]
    pub fn new(tab: usize) -> Self {
        Self {
            tab,
            ui: None,
            held: None,
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

/// Sets the UI scale, or with `text` the text scale, which `Settings`
/// clamps to its range, ignoring a value that is not finite.
pub fn set_scale(world: &mut World, text: bool, scale: f32) {
    change(world, |settings| match text {
        true => settings.set_text_scale(Some(scale)),
        false => settings.set_ui_scale(Some(scale)),
    });
}

/// A window mode or size change: applied, then kept only when the player
/// says so within 10 s.
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
        let tree = world.resource_mut::<UiTree>();
        let fullscreen = display.display_mode != DisplayMode::Windowed;
        let volume = |bus: Option<AudioBus>| {
            let set = bus.map_or(settings.master_volume(), |bus| settings.bus_volume(bus));
            set.unwrap_or(1.0) * 100.0
        };
        for (&(row, ..), &id) in ROWS.iter().zip(&ui.rows) {
            let value = match row {
                Row::Mode => {
                    let mode = if fullscreen { "Fullscreen" } else { "Windowed" };
                    _ = tree.set_text(id, mode);
                    continue;
                }
                Row::Size => {
                    let Resolution { width, height } = display.resolution;
                    tree.set_text(id, format!("{width}×{height}").as_str());
                    tree.set_enabled(id, !fullscreen);
                    continue;
                }
                Row::Cap => {
                    let cap = display
                        .frame_rate_cap
                        .map_or("Off".into(), |fps| fps.to_string());
                    _ = tree.set_text(id, cap.as_str());
                    continue;
                }
                Row::Difficulty => {
                    tree.set_text(
                        id,
                        difficulty.map_or("Chosen at New Game", Difficulty::label),
                    );
                    tree.set_enabled(id, difficulty.is_some());
                    continue;
                }
                Row::Keys | Row::ResetKeys => continue,
                Row::Vsync => UiValue::Bool(display.vsync),
                Row::MuteUnfocused => UiValue::Bool(options.mute_unfocused),
                Row::Damage => UiValue::Bool(options.damage_numbers),
                Row::PauseUnfocused => UiValue::Bool(options.pause_unfocused),
                Row::Master => UiValue::Number(volume(None)),
                Row::Music => UiValue::Number(volume(Some(AudioBus::Music))),
                Row::Effects => UiValue::Number(volume(Some(AudioBus::Effects))),
                Row::UiScale => UiValue::Number(settings.ui_scale().unwrap_or(1.0)),
                Row::TextScale => UiValue::Number(settings.text_scale().unwrap_or(1.0)),
                Row::Shake => UiValue::Number(options.screen_shake() * 100.0),
            };
            match value {
                UiValue::Bool(on) => _ = tree.set_toggled(id, on),
                _ => _ = tree.set_slider_value(id, value.as_number().unwrap_or(0.0)),
            }
        }
    }

    /// A row's button activated.
    fn activate(world: &mut World, row: Row) {
        let display = display(world);
        match row {
            Row::Mode => {
                let mode = match display.display_mode {
                    DisplayMode::Windowed => DisplayMode::BorderlessFullscreen,
                    _ => DisplayMode::Windowed,
                };
                change_display(world, |settings| settings.set_display_mode(Some(mode)));
            }
            Row::Size => {
                let (width, height) = next(
                    &SIZES,
                    (display.resolution.width, display.resolution.height),
                );
                let size = Resolution { width, height };
                change_display(world, |settings| settings.set_resolution(Some(size)));
            }
            Row::Cap => {
                let fps = NonZeroU32::new(next(&CAPS, display.frame_rate_cap.unwrap_or(0)));
                let cap = fps.map_or(FrameRateCap::Uncapped, FrameRateCap::Fps);
                change(world, |settings| settings.set_frame_rate_cap(Some(cap)));
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

    /// A slider or toggle took `value`.
    fn set(world: &mut World, row: Row, value: UiValue) {
        let (number, on) = (
            value.as_number().unwrap_or(0.0),
            value.as_bool().unwrap_or(false),
        );
        match row {
            Row::Vsync => change(world, |settings| settings.set_vsync(Some(on))),
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
                    Row::Shake => options.set_screen_shake(number / 100.0),
                    _ => {}
                }
                options::apply(world);
            }
        }
    }

    /// Returns the open tab's values to unset or default.
    fn reset(&self, world: &mut World) {
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
        match self.tab {
            1 => world.resource_mut::<GameOptions>().mute_unfocused = defaults.mute_unfocused,
            3 => keybindings::reset(world, None),
            GAMEPLAY_TAB => {
                let options = world.resource_mut::<GameOptions>();
                let kept = options.mute_unfocused;
                *options = defaults;
                options.mute_unfocused = kept;
                options::apply(world);
            }
            _ => {}
        }
        self.refresh(world);
    }

    /// Writes `Settings` and the options, then closes.
    fn leave(world: &mut World) {
        if let Err(error) = world.resource::<Settings>().save() {
            log::warn!("Settings not saved: {error}");
        }
        options::save(world);
        pop(world);
    }
}

impl GameState for SettingsState {
    fn id(&self) -> StateId {
        SETTINGS
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        let tree = ctx.world.resource_mut::<UiTree>();
        let root = widgets::screen(tree, theme::WIDE_WIDTH);
        widgets::heading(tree, root, "Settings");
        let (tabs, bars) = widgets::tab_row(tree, root, &TABS);
        let panels: Vec<WidgetId> = TABS.iter().map(|_| widgets::column(tree, root)).collect();
        let rows = ROWS
            .iter()
            .map(|&(_, tab, caption, kind)| match kind {
                Kind::Choice => widgets::choice_row(tree, panels[tab], caption, ""),
                Kind::Toggle => widgets::toggle_row(tree, panels[tab], caption, false),
                Kind::Button => widgets::button(tree, panels[tab], caption),
                Kind::Slider(least, most, step) => {
                    let range = SliderRange::new(least, most).with_step(step);
                    widgets::slider_row(tree, panels[tab], caption, range)
                }
            })
            .collect();
        let actions = widgets::row(tree, root);
        let reset = widgets::button(tree, actions, "Reset tab");
        let back = widgets::button(tree, actions, "Back");
        theme::slide_in(tree, root);
        let ui = Ui {
            root,
            tabs,
            bars,
            panels,
            rows,
            reset,
            back,
        };
        self.ui = Some(ui);
        self.select(tree, self.tab);
        stack(ctx.world).own_root(root, RootCover::Hide);
        self.refresh(ctx.world);
    }

    fn on_exit(&mut self, _ctx: &mut StateContext) {}

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
        let step = match (
            pressed(world, "ui_next", false),
            pressed(world, "ui_prev", false),
        ) {
            (true, false) => 1,
            (false, true) => TABS.len() - 1,
            _ => 0,
        };
        if step > 0 {
            let tab = (self.tab + step) % TABS.len();
            self.select(world.resource_mut::<UiTree>(), tab);
        }
        let events = events(world);
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
                UiEvent::Activated { widget, .. } => {
                    if let Some(row) = row_of(widget) {
                        Self::activate(world, row);
                        self.refresh(world);
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
                    Self::set(
                        world,
                        row_of(widget).unwrap_or(Row::UiScale),
                        UiValue::Number(value),
                    );
                }
                UiEvent::ValueChanged { widget, value, .. } if self.held != Some(widget) => {
                    if let Some(row) = row_of(widget) {
                        Self::set(world, row, value);
                    }
                }
                _ => {}
            }
        }
    }
}
