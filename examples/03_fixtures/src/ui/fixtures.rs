//! Driving a UI from scripts: `TUNGSTEN_UI_LAB_FIXTURE`,
//! `TUNGSTEN_UI_LAB_POST` and `TUNGSTEN_UI_LAB_BOUNDS` parsed, and the systems that run each scripted
//! fixture by frame at smoke's pinned dt, for the smoke rows, the pixel
//! tests and the perf sitting. A script acts at its frame or, where a
//! transition may still run, at the first frame after it with none
//! (`request_*_transition` refuses a request while one runs), and prints a
//! line per action that the smoke checks read.
//!
//! - `default`, `hidden`, `labels`, `debug`: the paint-path screen
//!   ([`crate::ui::screens`]); `labels` adds [`crate::ui::labels`]'s fixed set.
//! - `menus`: the play state through pause, settings, a cancelled capture
//!   and back ([`menus_script_system`]).
//! - `menu`, `menu-prewarm`, `menu-cold`: the budget menu
//!   ([`crate::ui::budget`], [`menu_open_system`]).
//! - `debug`: every debug view open ([`open_debug_views`],
//!   [`debug_fixture_system`]).
//! - `display`: the settings screen through resizes and display modes
//!   ([`display_script_system`]; not a smoke row, since it flips the screen).

use std::fmt::Write as _;

use glam::Vec2;
use tungsten::core::components::{Tag, Transform};
use tungsten::core::ui::UiTree;
use tungsten::core::{DisplayMode, DisplayState, Settings, World};
use tungsten::{
    DebugHud, HudRow, InspectorState, LogConsole, StateStack, SystemTimingOverlay, UiInspector,
};

use crate::ui::budget::{BudgetMenu, MENU_HIDE, MENU_REOPEN, MENU_SHOW};
use crate::ui::hud::LabHud;
use crate::ui::inventory::InventoryState;
use crate::ui::menus::{self, PauseState, SettingsMenu, SettingsState, stack};
use crate::ui::theme;

/// `TUNGSTEN_UI_LAB_FIXTURE`, parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fixture {
    /// Not set: interactive run with the debug HUD.
    Off,
    /// The lab, no debug HUD.
    Default,
    /// No roots at all.
    Empty,
    /// The lab with the game layers hidden at startup.
    Hidden,
    /// The lab with a fixed set of world labels.
    Labels,
    /// The play state, scripted through pause, settings and back.
    Menus,
    /// The 100-widget menu, shown from the first frame.
    Menu,
    /// That menu hidden and prewarmed, shown at frame 240.
    MenuPrewarm,
    /// That menu hidden, not prewarmed, shown at frame 240.
    MenuCold,
    /// The lab under every debug view.
    Debug,
    /// The settings screen through resizes and display modes.
    Display,
}

impl Fixture {
    /// `TUNGSTEN_UI_LAB_FIXTURE`; unset is [`Fixture::Off`].
    pub fn from_env() -> anyhow::Result<Self> {
        let Ok(name) = std::env::var("TUNGSTEN_UI_LAB_FIXTURE") else {
            return Ok(Self::Off);
        };
        match name.as_str() {
            "default" => Ok(Self::Default),
            "empty" => Ok(Self::Empty),
            "hidden" => Ok(Self::Hidden),
            "labels" => Ok(Self::Labels),
            "menus" => Ok(Self::Menus),
            "menu" => Ok(Self::Menu),
            "menu-prewarm" => Ok(Self::MenuPrewarm),
            "menu-cold" => Ok(Self::MenuCold),
            "debug" => Ok(Self::Debug),
            "display" => Ok(Self::Display),
            other => anyhow::bail!(
                "invalid TUNGSTEN_UI_LAB_FIXTURE='{other}': expected one of default, empty, hidden, labels, menus, menu, menu-prewarm, menu-cold, debug, display"
            ),
        }
    }
}

/// `TUNGSTEN_UI_LAB_POST`, parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Post {
    /// Unset or `off`: no post pass of the lab's.
    Off,
    /// A vignette appended to the post stack.
    Vignette,
}

impl Post {
    /// `TUNGSTEN_UI_LAB_POST`; unset is [`Post::Off`].
    pub fn from_env() -> anyhow::Result<Self> {
        match std::env::var("TUNGSTEN_UI_LAB_POST").as_deref() {
            Err(_) | Ok("off") => Ok(Self::Off),
            Ok("vignette") => Ok(Self::Vignette),
            Ok(other) => {
                anyhow::bail!(
                    "invalid TUNGSTEN_UI_LAB_POST='{other}': expected one of off, vignette"
                )
            }
        }
    }
}

/// `TUNGSTEN_UI_LAB_BOUNDS`: `1` shows the layout bounds from the first
/// frame, for the bounds captures; unset or `0` does not.
pub fn bounds_from_env() -> anyhow::Result<bool> {
    match std::env::var("TUNGSTEN_UI_LAB_BOUNDS").as_deref() {
        Err(_) | Ok("0") => Ok(false),
        Ok("1") => Ok(true),
        Ok(other) => anyhow::bail!("invalid TUNGSTEN_UI_LAB_BOUNDS='{other}': expected 0 or 1"),
    }
}

/// Counts frames for the scripted fixtures; a `World` resource.
#[derive(Debug, Default)]
pub struct FixtureClock {
    /// Frames counted, the current one included.
    pub frame: u64,
    /// The `menus` script's steps after its fixed frames: the inventory
    /// opened, closed, and play settled again.
    step: u8,
}

// ---- menus ----

/// The `menus` fixture's script, by frame at smoke's pinned dt: pause at
/// frame 10, settings at 40, a capture started at 60 and cancelled at 70,
/// Back at 80 and Resume at 110; then, from frame 120, the inventory opened
/// from play and closed, each at the first frame with no transition
/// running, and `ui_lab menus: frame <F>, top 'play', settled` once play is
/// back on top with none running, within the smoke row's 150 frames.
pub fn menus_script_system(world: &mut World) {
    let Some(clock) = world.get_resource_mut::<FixtureClock>() else {
        return;
    };
    clock.frame += 1;
    let (frame, step) = (clock.frame, clock.step);
    if frame >= 120 {
        let states = world.resource::<StateStack>();
        if states.is_transitioning() {
            return;
        }
        let top = states.active_id().unwrap_or("");
        match (step, top) {
            (0, "play") => {
                stack(world).request_push_transition(InventoryState, theme::fade());
                println!("ui_lab menus: frame {frame}, top '{top}'");
            }
            (1, "inventory") => {
                stack(world).request_pop_transition(theme::fade());
                println!("ui_lab menus: frame {frame}, top '{top}'");
            }
            (2, "play") => println!("ui_lab menus: frame {frame}, top '{top}', settled"),
            _ => return,
        }
        world.resource_mut::<FixtureClock>().step += 1;
        return;
    }
    match frame {
        10 => {
            stack(world).request_push_transition(PauseState, theme::fade());
        }
        40 => {
            stack(world).request_push_transition(SettingsState, theme::fade());
        }
        60 => {
            if let Some(rebind) = world.get_resource::<SettingsMenu>().map(|m| m.rebind) {
                world
                    .resource_mut::<tungsten::core::ui::UiRouter>()
                    .capture_next_input(rebind);
            }
        }
        70 => world
            .resource_mut::<tungsten::core::ui::UiRouter>()
            .cancel_capture(),
        80 | 110 => {
            stack(world).request_pop_transition(theme::fade());
        }
        _ => return,
    }
    let top = world.resource::<StateStack>().active_id().unwrap_or("");
    println!("ui_lab menus: frame {frame}, top '{top}'");
}

// ---- menu-prewarm, menu-cold ----

/// Shows the hidden budget menu at frame [`MENU_SHOW`], hides it at
/// [`MENU_HIDE`] and shows it again at [`MENU_REOPEN`], printing each
/// frame index for the first-open check.
pub fn menu_open_system(world: &mut World) {
    let Some(clock) = world.get_resource_mut::<FixtureClock>() else {
        return;
    };
    clock.frame += 1;
    let frame = clock.frame;
    let Some(root) = world.get_resource::<BudgetMenu>().map(|menu| menu.root) else {
        return;
    };
    let (visibility, verb) = match frame {
        MENU_SHOW => (tungsten::core::ui::Visibility::Visible, "shown"),
        MENU_HIDE => (tungsten::core::ui::Visibility::Hidden, "hidden"),
        MENU_REOPEN => (tungsten::core::ui::Visibility::Visible, "reopened"),
        _ => return,
    };
    world
        .resource_mut::<UiTree>()
        .set_visibility(root, visibility);
    println!("ui_lab menu: {verb} at frame {frame}");
}

// ---- debug ----

/// Opens the timing overlay, the inspector over a tagged entity it selects,
/// the log console and the UI inspector over the lab's counter; adds the
/// HUD a 120-character row, which ends in `…`, and logs one `ERROR`, one
/// `WARN` and one `INFO` record for the console. The HUD opens with the
/// fixture.
pub fn open_debug_views(world: &mut World) {
    let hero = world.spawn_with((
        Tag::new("lab_hero"),
        Transform::from_position(Vec2::new(64.0, 48.0)),
    ));
    if let Some(overlay) = world.get_resource_mut::<SystemTimingOverlay>() {
        overlay.enabled = true;
    }
    if let Some(inspector) = world.get_resource_mut::<InspectorState>() {
        inspector.enabled = true;
        inspector.selected = Some(hero);
    }
    if let Some(console) = world.get_resource_mut::<LogConsole>() {
        console.enabled = true;
    }
    if let Some(hud) = world.get_resource_mut::<DebugHud>() {
        hud.add_row(|_| {
            vec![HudRow {
                label: "lab",
                value: "a row longer than the HUD is wide: ".repeat(4)[..120].to_string(),
            }]
        });
    }
    let counter = world
        .get_resource::<LabHud>()
        .map(|lab_hud| lab_hud.counter);
    if let Some(inspector) = world.get_resource_mut::<UiInspector>() {
        inspector.enabled = true;
        inspector.picked = counter;
    }
    log::error!("ui lab: an error for the log console");
    log::warn!("ui lab: a warning for the log console");
    log::info!("ui lab: a note for the log console");
}

/// At frame 30, scrolls the timing overlay's body, as the wheel would.
pub fn debug_fixture_system(world: &mut World) {
    if world.get_resource::<LabHud>().map(|lab_hud| lab_hud.frames) != Some(30) {
        return;
    }
    let overlay = world
        .get_resource::<SystemTimingOverlay>()
        .and_then(SystemTimingOverlay::window);
    let Some(tree) = world.get_resource_mut::<UiTree>() else {
        return;
    };
    let body = overlay.and_then(|root| {
        tree.children(root)
            .iter()
            .copied()
            .find(|child| tree.scroll(*child).is_some())
    });
    if let Some(body) = body {
        tree.set_scroll(body, 120.0);
        println!(
            "ui lab: scrolled the timing overlay to {:?}",
            tree.scroll(body)
        );
    }
}

// ---- display ----

/// The `display` fixture's phases (M59): A requests 1600×900 as a resize by
/// hand ends; M waits for the window to change on its own (a maximize from
/// outside); B steps the volume, an unrelated apply; C1–C3 cycle the
/// display mode through borderless and exclusive back to windowed, C2 with
/// three volume steps; D leaves the settings screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DisplayPhase {
    Open,
    A,
    M,
    B,
    C1,
    C2,
    C3,
    D,
    Done,
}

impl DisplayPhase {
    fn name(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::A => "A",
            Self::M => "M",
            Self::B => "B",
            Self::C1 => "C1",
            Self::C2 => "C2",
            Self::C3 => "C3",
            Self::D => "D",
            Self::Done => "done",
        }
    }
}

/// Frames each phase holds after its end line, before the next starts.
const DISPLAY_HOLD: u64 = 30;

/// The `display` fixture's script state; a `World` resource.
#[derive(Debug)]
pub struct DisplayScript {
    frame: u64,
    phase: DisplayPhase,
    /// Frames since the phase started.
    at: u64,
    /// Frames left in the hold after a phase's end line.
    hold: Option<u64>,
    maximize_wait: bool,
    /// The window size phase M started at.
    waited_from: Option<(u32, u32)>,
    /// The state the last line printed, frame and phase aside.
    last: String,
}

impl DisplayScript {
    /// A script that runs phase M before B when `maximize_wait` is set.
    #[must_use]
    pub fn new(maximize_wait: bool) -> Self {
        Self {
            frame: 0,
            phase: DisplayPhase::Open,
            at: 0,
            hold: None,
            maximize_wait,
            waited_from: None,
            last: String::new(),
        }
    }

    fn next_phase(&self) -> DisplayPhase {
        match self.phase {
            DisplayPhase::A if self.maximize_wait => DisplayPhase::M,
            DisplayPhase::A | DisplayPhase::M => DisplayPhase::B,
            DisplayPhase::Open => DisplayPhase::A,
            DisplayPhase::B => DisplayPhase::C1,
            DisplayPhase::C1 => DisplayPhase::C2,
            DisplayPhase::C2 => DisplayPhase::C3,
            DisplayPhase::C3 => DisplayPhase::D,
            DisplayPhase::D | DisplayPhase::Done => DisplayPhase::Done,
        }
    }
}

/// The engine's view of the window and the settings screen: the window
/// size, the display state, the settings' mode, the scale, the screen's
/// root and its centre's offset from the viewport's, in UI units.
fn display_snapshot(world: &World) -> String {
    let window = world
        .get_resource::<tungsten::WindowSize>()
        .map_or((0, 0), |size| (size.width, size.height));
    let state = world.get_resource::<DisplayState>().copied();
    let settings = world
        .get_resource::<Settings>()
        .and_then(Settings::display_mode)
        .map_or("-", DisplayMode::as_str);
    let scale = world
        .get_resource::<tungsten::core::ScaleFactor>()
        .map_or(1.0, tungsten::core::ScaleFactor::get);
    let root = world
        .get_resource::<SettingsMenu>()
        .and_then(|menu| world.resource::<UiTree>().rect(menu.root));
    let mut line = format!("window {}x{}", window.0, window.1);
    match state {
        Some(state) => {
            let _ = write!(
                line,
                " state {} {}x{}",
                state.display_mode.as_str(),
                state.resolution.width,
                state.resolution.height
            );
        }
        None => line.push_str(" state -"),
    }
    let _ = write!(line, " settings {settings} scale {scale}");
    match root {
        Some(rect) => {
            let viewport = Vec2::new(window.0 as f32, window.1 as f32) / scale;
            let offset = (rect.min + rect.max) * 0.5 - viewport * 0.5;
            let _ = write!(
                line,
                " root ({:.1}, {:.1}) {:.1}x{:.1} offset ({:.1}, {:.1})",
                rect.min.x,
                rect.min.y,
                rect.width(),
                rect.height(),
                offset.x,
                offset.y
            );
        }
        None => line.push_str(" root - offset -"),
    }
    line
}

/// Runs `action` on the settings screen's menu while it is on the stack.
fn with_settings_menu(world: &mut World, action: impl FnOnce(&mut World, &mut SettingsMenu)) {
    if let Some(mut menu) = world.remove_resource::<SettingsMenu>() {
        action(world, &mut menu);
        world.insert_resource(menu);
    }
}

/// The start of `phase`.
fn start_display_phase(world: &mut World, phase: DisplayPhase) {
    match phase {
        DisplayPhase::A => {
            let mut requested = world
                .get_resource::<DisplayState>()
                .copied()
                .unwrap_or_default();
            requested.display_mode = DisplayMode::Windowed;
            requested.resolution = tungsten::core::Resolution {
                width: 1600,
                height: 900,
            };
            if let Err(error) = tungsten::request_display_settings(world, requested) {
                println!("ui_lab display: request rejected: {error}");
            }
        }
        DisplayPhase::M => println!("ui_lab display: phase M waiting"),
        DisplayPhase::B => with_settings_menu(world, |world, menu| menus::step_volume(world, menu)),
        DisplayPhase::C1 | DisplayPhase::C2 | DisplayPhase::C3 => {
            with_settings_menu(world, menus::cycle_display_mode);
        }
        DisplayPhase::D => {
            stack(world).request_pop_transition(theme::fade());
        }
        DisplayPhase::Open | DisplayPhase::Done => {}
    }
}

/// Whether `phase`, `at` frames in, is over.
fn display_phase_over(world: &World, script: &DisplayScript) -> bool {
    let window = world
        .get_resource::<tungsten::WindowSize>()
        .map(|size| (size.width, size.height));
    match script.phase {
        DisplayPhase::A => window == Some((1600, 900)) || script.at >= 120,
        DisplayPhase::M => window != script.waited_from || script.at >= 600,
        DisplayPhase::B => script.at >= 60,
        DisplayPhase::C1 | DisplayPhase::C2 | DisplayPhase::C3 => script.at >= 90,
        DisplayPhase::D => {
            let stack = world.resource::<StateStack>();
            (stack.active_id() == Some("pause") && !stack.is_transitioning()) || script.at >= 90
        }
        DisplayPhase::Open | DisplayPhase::Done => false,
    }
}

/// The `display` fixture's script (M59, not a smoke row: it flips the
/// screen): pause at frame 10, settings from frame 30 once no transition
/// runs, then phases A, M (with `TUNGSTEN_UI_LAB_MAXIMIZE_WAIT=1`), B,
/// C1–C3 and D, each ending in `ui_lab display: phase <P> end` with the
/// engine's view of the window and holding 30 frames before the next.
/// Every change between prints `ui_lab display: frame <F> phase <P> window
/// <W>x<H> state <mode> <W>x<H> settings <mode or -> scale <S> root (<x>,
/// <y>) <w>x<h> offset (<dx>, <dy>)`.
pub fn display_script_system(world: &mut World) {
    let Some(mut script) = world.remove_resource::<DisplayScript>() else {
        return;
    };
    script.frame += 1;
    let frame = script.frame;
    let transitioning = world.resource::<StateStack>().is_transitioning();
    let top = world.resource::<StateStack>().active_id().unwrap_or("");
    if script.phase == DisplayPhase::Open && !transitioning {
        if top == "play" && frame >= 10 {
            stack(world).request_push_transition(PauseState, theme::fade());
        } else if top == "pause" && frame >= 30 {
            stack(world).request_push_transition(SettingsState, theme::fade());
        } else if top == "settings" {
            script.phase = DisplayPhase::A;
            script.at = 0;
            start_display_phase(world, DisplayPhase::A);
        }
    } else if let Some(left) = script.hold {
        if left > 1 {
            script.hold = Some(left - 1);
        } else {
            script.hold = None;
            script.phase = script.next_phase();
            script.at = 0;
            script.waited_from = world
                .get_resource::<tungsten::WindowSize>()
                .map(|size| (size.width, size.height));
            start_display_phase(world, script.phase);
        }
    } else if script.phase != DisplayPhase::Open && script.phase != DisplayPhase::Done {
        script.at += 1;
        if script.phase == DisplayPhase::C2 && matches!(script.at, 10 | 20 | 30) {
            with_settings_menu(world, |world, menu| menus::step_volume(world, menu));
        }
        if display_phase_over(world, &script) {
            println!(
                "ui_lab display: phase {} end frame {frame} {}",
                script.phase.name(),
                display_snapshot(world)
            );
            script.hold = Some(DISPLAY_HOLD);
        }
    }
    let snapshot = display_snapshot(world);
    if snapshot != script.last {
        println!(
            "ui_lab display: frame {frame} phase {} {snapshot}",
            script.phase.name()
        );
        script.last = snapshot;
    }
    world.insert_resource(script);
}
