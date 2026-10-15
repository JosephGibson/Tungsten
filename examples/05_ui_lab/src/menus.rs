//! The lab's gameplay screens (M56): a `play` state owning the lab's roots
//! and a HUD row (a live timer written without allocating and a
//! binding-aware prompt), a `pause` state's modal menu and a `settings`
//! state's screen (display mode, UI and text scale, master volume, one
//! rebind, Back), each pushed and popped behind a fade. Each state owns its
//! roots on the state stack, so a covered screen hides or freezes and the
//! transition fades every owned root with the frame. The menu fixtures'
//! 100-widget menu, the budget's subject, lives here too, with the script
//! the `menus` smoke row runs.

use std::fmt::Write as _;

use glam::Vec2;
use tungsten::core::ui::{
    Align, Anchor, CursorIcon, LayoutStyle, SliderRange, StyleOverrides, TextOverrides, UiEvent,
    UiLayer, UiRouter, UiTrack, UiTree, UiValue, WidgetId,
};
use tungsten::core::{
    ActionMap, Binding, DisplayMode, DisplayState, Easing, EventQueue, GamepadButton, RoutedInput,
    Settings, Time, World,
};
use tungsten::{
    GameState, RootCover, StateContext, StateId, StateStack, Transition, TransitionEffect,
    apply_settings,
};

/// The fade every screen change takes, each way, in seconds.
pub const FADE_SECS: f32 = 0.15;
/// The action the play state pauses on: P in the engine defaults, and the
/// pad's Start, which [`add_pause_pad_binding`] adds in code.
pub const PAUSE_ACTION: &str = "state_pause";
/// The action the settings screen rebinds.
pub const REBIND_ACTION: &str = "jump";

/// A fade to black and back, [`FADE_SECS`] each way.
#[must_use]
pub fn fade() -> Transition {
    Transition::new(
        TransitionEffect::Fade {
            color: [0.0, 0.0, 0.0, 1.0],
        },
        FADE_SECS,
    )
}

/// Adds the pad's Start to [`PAUSE_ACTION`]'s bindings, once.
pub fn add_pause_pad_binding(world: &mut World) {
    let Some(map) = world.get_resource_mut::<ActionMap>() else {
        return;
    };
    let start = Binding::GamepadButton {
        button: GamepadButton::Start,
    };
    let mut bindings = map.bindings(PAUSE_ACTION).to_vec();
    if !bindings.contains(&start) {
        bindings.push(start);
        map.replace_bindings(PAUSE_ACTION, bindings);
    }
}

fn transparent() -> StyleOverrides {
    StyleOverrides {
        background: Some([0, 0, 0, 0]),
        border_width: Some(0.0),
        ..StyleOverrides::default()
    }
}

fn heading(tree: &mut UiTree, parent: WidgetId, text: &str, size: f32) -> WidgetId {
    let label = tree.label(parent, text);
    tree.set_overrides(
        label,
        StyleOverrides {
            text: TextOverrides {
                size: Some(size),
                line_height: Some(size * 1.3),
                weight: Some(700),
                ..TextOverrides::default()
            },
            ..StyleOverrides::default()
        },
    );
    label
}

fn button(tree: &mut UiTree, parent: WidgetId, text: &str) -> WidgetId {
    let id = tree.button(parent, text);
    tree.set_cursor(id, Some(CursorIcon::Pointer));
    id
}

/// A modal screen root on the popup layer, centred.
fn screen_root(tree: &mut UiTree, width: f32) -> WidgetId {
    let mut style = LayoutStyle::column()
        .with_padding(20.0)
        .with_gap(10.0)
        .with_size(
            tungsten::core::ui::Sizing::Px(width),
            tungsten::core::ui::Sizing::Auto,
        )
        .anchored(Anchor::Center, Vec2::ZERO);
    style.align_items = Align::Stretch;
    let root = tree.add_root(UiLayer::GamePopup, style);
    tree.set_modal(root, true);
    root
}

/// Slides a new screen up into place over 0.2 real seconds.
fn slide_in(tree: &mut UiTree, root: WidgetId) {
    tree.animate(
        root,
        UiTrack::translation(Vec2::new(0.0, 24.0), Vec2::ZERO, 0.2).with_easing(Easing::QuadOut),
    );
}

fn activated(world: &World, widget: WidgetId) -> bool {
    world
        .get_resource::<EventQueue<UiEvent>>()
        .is_some_and(|queue| queue.activated(widget))
}

fn cancelled(world: &World, scope: WidgetId) -> bool {
    world
        .get_resource::<EventQueue<UiEvent>>()
        .is_some_and(|queue| {
            queue
                .iter_current()
                .any(|event| matches!(event, UiEvent::Cancel { scope: s, .. } if *s == scope))
        })
}

fn stack(world: &mut World) -> &mut StateStack {
    world.resource_mut::<StateStack>()
}

// ---- play ----

/// The play state's HUD row: a `World` resource while `play` is on the
/// stack.
#[derive(Debug)]
pub struct PlayHud {
    /// The row's root.
    pub root: WidgetId,
    /// Game seconds, written with `set_plain_text` from [`Self::text`].
    pub timer: WidgetId,
    /// "Press {action:state_pause} to pause".
    pub prompt: WidgetId,
    text: String,
}

/// The game screen: owns the lab's roots (`Show`, so they stay drawn and
/// frozen under the pause menu) and a HUD row, counts game time, and pushes
/// [`PauseState`] behind a fade on [`PAUSE_ACTION`]. Pauses the game clock
/// while covered.
pub struct PlayState {
    roots: Vec<WidgetId>,
}

impl PlayState {
    /// A play state that will own `roots`, the lab's.
    #[must_use]
    pub fn new(roots: Vec<WidgetId>) -> Self {
        Self { roots }
    }
}

impl GameState for PlayState {
    fn id(&self) -> StateId {
        "play"
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        let tree = ctx.world.resource_mut::<UiTree>();
        let mut style = LayoutStyle::row()
            .with_padding(8.0)
            .with_gap(16.0)
            .anchored(Anchor::BottomStart, Vec2::new(16.0, 16.0));
        style.align_items = Align::Center;
        let root = tree.add_root(UiLayer::Game, style);
        tree.set_hit_testable(root, false);
        let timer = tree.label(root, "0.0 s");
        tree.set_overrides(
            timer,
            StyleOverrides {
                text: TextOverrides {
                    family: Some("mono".to_owned()),
                    ..TextOverrides::default()
                },
                ..StyleOverrides::default()
            },
        );
        let prompt = tree.label(root, "");
        tree.set_prompt(prompt, "Press {action:state_pause} to pause");
        let stack = stack(ctx.world);
        for owned in self.roots.iter().copied().chain([root]) {
            stack.own_root(owned, RootCover::Show);
        }
        ctx.world.insert_resource(PlayHud {
            root,
            timer,
            prompt,
            text: String::with_capacity(16),
        });
    }

    fn on_exit(&mut self, ctx: &mut StateContext) {
        ctx.world.remove_resource::<PlayHud>();
    }

    fn on_pause(&mut self, ctx: &mut StateContext) {
        ctx.world.resource_mut::<Time>().pause();
    }

    fn on_resume(&mut self, ctx: &mut StateContext) {
        ctx.world.resource_mut::<Time>().resume();
    }

    fn update(&mut self, world: &mut World) {
        let seconds = world.resource::<Time>().elapsed();
        if let Some(mut hud) = world.remove_resource::<PlayHud>() {
            hud.text.clear();
            let _ = write!(hud.text, "{seconds:.1} s");
            world
                .resource_mut::<UiTree>()
                .set_plain_text(hud.timer, &hud.text);
            world.insert_resource(hud);
        }
        let pressed = world
            .get_resource::<ActionMap>()
            .zip(world.get_resource::<RoutedInput>())
            .is_some_and(|(map, routed)| map.just_pressed(routed, PAUSE_ACTION));
        if pressed && !world.resource::<StateStack>().is_transitioning() {
            stack(world).request_push_transition(PauseState, fade());
        }
    }
}

// ---- pause ----

/// The pause menu: a `World` resource while `pause` is on the stack.
#[derive(Debug)]
pub struct PauseMenu {
    /// The modal root.
    pub root: WidgetId,
    /// "Resume", focused at the start.
    pub resume: WidgetId,
    /// "Settings".
    pub settings: WidgetId,
    /// The prompt row.
    pub prompt: WidgetId,
}

/// The pause screen: a modal "Paused" menu (Resume, Settings) with a prompt
/// row, owned with `Hide`; Resume or `ui_cancel` pops it and Settings
/// pushes [`SettingsState`], each behind a fade.
pub struct PauseState;

impl GameState for PauseState {
    fn id(&self) -> StateId {
        "pause"
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        let tree = ctx.world.resource_mut::<UiTree>();
        let root = screen_root(tree, 260.0);
        heading(tree, root, "Paused", 24.0);
        let resume = button(tree, root, "Resume");
        let settings = button(tree, root, "Settings");
        let prompt = tree.label(root, "");
        tree.set_prompt(
            prompt,
            "{action:ui_accept} select   {action:ui_cancel} resume",
        );
        tree.focus(resume);
        slide_in(tree, root);
        stack(ctx.world).own_root(root, RootCover::Hide);
        ctx.world.insert_resource(PauseMenu {
            root,
            resume,
            settings,
            prompt,
        });
    }

    fn on_exit(&mut self, ctx: &mut StateContext) {
        ctx.world.remove_resource::<PauseMenu>();
    }

    fn on_resume(&mut self, ctx: &mut StateContext) {
        if let Some(settings) = ctx.world.get_resource::<PauseMenu>().map(|m| m.settings) {
            ctx.world.resource_mut::<UiTree>().focus(settings);
        }
    }

    fn update(&mut self, world: &mut World) {
        let Some(menu) = world.get_resource::<PauseMenu>() else {
            return;
        };
        let (root, resume, settings) = (menu.root, menu.resume, menu.settings);
        if world.resource::<StateStack>().is_transitioning() {
            return;
        }
        if activated(world, resume) || cancelled(world, root) {
            stack(world).request_pop_transition(fade());
        } else if activated(world, settings) {
            stack(world).request_push_transition(SettingsState, fade());
        }
    }
}

// ---- settings ----

/// The settings screen: a `World` resource while `settings` is on the
/// stack.
#[derive(Debug)]
pub struct SettingsMenu {
    /// The modal root.
    pub root: WidgetId,
    /// Cycles windowed, borderless and exclusive fullscreen.
    pub display_mode: WidgetId,
    /// The UI scale, 0.5 to 2 by 0.25, applied at the pointer's release or
    /// a key step.
    pub ui_scale: WidgetId,
    /// The text scale, 0.75 to 2 by 0.25, applied as the UI scale is.
    pub text_scale: WidgetId,
    /// The master volume, 0 to 100 by 5, applied live.
    pub volume: WidgetId,
    /// Shows [`REBIND_ACTION`]'s binding and captures a new one.
    pub rebind: WidgetId,
    /// Saves the settings and pops.
    pub back: WidgetId,
    /// The display requests the screen has made.
    pub display_requests: u32,
    /// The scale slider the pointer holds, and the value it would set.
    dragging: Option<(WidgetId, Option<f32>)>,
}

/// The settings screen: display mode, UI and text scale, master volume, one
/// rebind and Back, which saves the settings and pops behind a fade, as
/// `ui_cancel` does.
pub struct SettingsState;

const MODES: [DisplayMode; 3] = [
    DisplayMode::Windowed,
    DisplayMode::BorderlessFullscreen,
    DisplayMode::ExclusiveFullscreen,
];

fn mode_text(mode: DisplayMode) -> &'static str {
    match mode {
        DisplayMode::Windowed => "Display: windowed",
        DisplayMode::BorderlessFullscreen => "Display: borderless",
        DisplayMode::ExclusiveFullscreen => "Display: exclusive",
    }
}

/// A row of the settings screen: a caption filling the space before the
/// control.
fn setting_row(tree: &mut UiTree, root: WidgetId, caption: &str) -> WidgetId {
    let mut style = LayoutStyle::row().with_gap(8.0);
    style.align_items = Align::Center;
    let row = tree.panel(root, style);
    tree.set_overrides(row, transparent());
    let label = tree.label(row, caption);
    tree.set_layout_style(
        label,
        LayoutStyle::default().with_size(
            tungsten::core::ui::Sizing::Fill,
            tungsten::core::ui::Sizing::Auto,
        ),
    );
    row
}

fn slider(
    tree: &mut UiTree,
    row: WidgetId,
    range: SliderRange,
    value: f32,
    name: &str,
) -> WidgetId {
    let id = tree.slider(row, range, value);
    tree.set_layout_style(
        id,
        LayoutStyle::default().with_size(
            tungsten::core::ui::Sizing::Px(140.0),
            tungsten::core::ui::Sizing::Auto,
        ),
    );
    tree.set_accessible_name(id, Some(name.to_owned()));
    tree.set_cursor(id, Some(CursorIcon::Pointer));
    id
}

const REBIND_PROMPT: &str = "{action:jump}";

impl GameState for SettingsState {
    fn id(&self) -> StateId {
        "settings"
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        let settings = ctx
            .world
            .get_resource::<Settings>()
            .cloned()
            .unwrap_or_default();
        let mode = settings.display_mode().unwrap_or_else(|| {
            ctx.world
                .get_resource::<DisplayState>()
                .map_or(DisplayMode::Windowed, |state| state.display_mode)
        });
        let tree = ctx.world.resource_mut::<UiTree>();
        let root = screen_root(tree, 380.0);
        heading(tree, root, "Settings", 24.0);
        let display_mode = button(tree, root, mode_text(mode));
        let row = setting_row(tree, root, "UI scale");
        let ui_scale = slider(
            tree,
            row,
            SliderRange::new(0.5, 2.0).with_step(0.25),
            settings.ui_scale().unwrap_or(1.0),
            "UI scale",
        );
        let row = setting_row(tree, root, "Text scale");
        let text_scale = slider(
            tree,
            row,
            SliderRange::new(0.75, 2.0).with_step(0.25),
            settings.text_scale().unwrap_or(1.0),
            "Text scale",
        );
        let row = setting_row(tree, root, "Volume");
        let volume = slider(
            tree,
            row,
            SliderRange::new(0.0, 100.0).with_step(5.0),
            settings.master_volume().unwrap_or(1.0) * 100.0,
            "Volume",
        );
        let row = setting_row(tree, root, "Jump");
        let rebind = button(tree, row, "");
        tree.set_prompt(rebind, REBIND_PROMPT);
        let back = button(tree, root, "Back");
        tree.focus(display_mode);
        slide_in(tree, root);
        stack(ctx.world).own_root(root, RootCover::Hide);
        ctx.world.insert_resource(SettingsMenu {
            root,
            display_mode,
            ui_scale,
            text_scale,
            volume,
            rebind,
            back,
            display_requests: 0,
            dragging: None,
        });
    }

    fn on_exit(&mut self, ctx: &mut StateContext) {
        if let Some(router) = ctx.world.get_resource_mut::<UiRouter>()
            && router.capturing().is_some()
        {
            router.cancel_capture();
        }
        ctx.world.remove_resource::<SettingsMenu>();
    }

    fn update(&mut self, world: &mut World) {
        let Some(mut menu) = world.remove_resource::<SettingsMenu>() else {
            return;
        };
        let events: Vec<UiEvent> = world
            .get_resource::<EventQueue<UiEvent>>()
            .map(|queue| queue.iter_current().copied().collect())
            .unwrap_or_default();
        let mut leave = false;
        for event in events {
            leave |= settings_event(world, &mut menu, event);
        }
        if leave && !world.resource::<StateStack>().is_transitioning() {
            if let Some(settings) = world.get_resource::<Settings>()
                && let Err(error) = settings.save()
            {
                println!("settings not saved: {error}");
            }
            stack(world).request_pop_transition(fade());
        }
        world.insert_resource(menu);
    }
}

/// Changes `Settings` with `change`, then applies them as a restart would.
fn change_settings(world: &mut World, change: impl FnOnce(&mut Settings)) {
    if !world.has_resource::<Settings>() {
        world.insert_resource(Settings::default());
    }
    change(world.resource_mut::<Settings>());
    apply_settings(world);
}

/// Applies a scale slider's value.
fn apply_scale(world: &mut World, menu: &SettingsMenu, slider: WidgetId, value: f32) {
    if slider == menu.ui_scale {
        change_settings(world, |settings| settings.set_ui_scale(Some(value)));
    } else if slider == menu.text_scale {
        change_settings(world, |settings| settings.set_text_scale(Some(value)));
    }
}

/// One routed event on the settings screen; `true` asks to leave.
fn settings_event(world: &mut World, menu: &mut SettingsMenu, event: UiEvent) -> bool {
    let scales = [menu.ui_scale, menu.text_scale];
    match event {
        UiEvent::Activated { widget, .. } if widget == menu.display_mode => {
            let current = world
                .get_resource::<Settings>()
                .and_then(Settings::display_mode)
                .or_else(|| {
                    world
                        .get_resource::<DisplayState>()
                        .map(|state| state.display_mode)
                })
                .unwrap_or(DisplayMode::Windowed);
            let at = MODES.iter().position(|mode| *mode == current).unwrap_or(0);
            let next = MODES[(at + 1) % MODES.len()];
            change_settings(world, |settings| settings.set_display_mode(Some(next)));
            menu.display_requests += 1;
            world
                .resource_mut::<UiTree>()
                .set_text(menu.display_mode, mode_text(next));
        }
        UiEvent::Activated { widget, .. } if widget == menu.rebind => {
            world
                .resource_mut::<UiRouter>()
                .capture_next_input(menu.rebind);
            world
                .resource_mut::<UiTree>()
                .set_text(menu.rebind, "Press a key or button");
        }
        UiEvent::Activated { widget, .. } if widget == menu.back => return true,
        UiEvent::Cancel { scope, .. } if scope == menu.root => return true,
        UiEvent::Pressed { widget, .. } if scales.contains(&widget) => {
            menu.dragging = Some((widget, None));
        }
        UiEvent::ValueChanged {
            widget,
            value: UiValue::Number(value),
            ..
        } if scales.contains(&widget) => match &mut menu.dragging {
            // Held: applied at the release, so a drag is not rescaled under
            // the pointer.
            Some((held, pending)) if *held == widget => *pending = Some(value),
            _ => apply_scale(world, menu, widget, value),
        },
        UiEvent::Released { widget, .. } | UiEvent::PressCancelled { widget, .. }
            if scales.contains(&widget) =>
        {
            if let Some((held, Some(value))) = menu.dragging.take()
                && held == widget
            {
                apply_scale(world, menu, widget, value);
            }
        }
        UiEvent::ValueChanged {
            widget,
            value: UiValue::Number(value),
            ..
        } if widget == menu.volume => {
            change_settings(world, |settings| {
                settings.set_master_volume(Some(value / 100.0));
            });
        }
        UiEvent::InputCaptured {
            widget, binding, ..
        } if widget == menu.rebind => {
            rebind(world, binding);
            world
                .resource_mut::<UiTree>()
                .set_prompt(menu.rebind, REBIND_PROMPT);
        }
        UiEvent::CaptureCancelled { widget, .. } if widget == menu.rebind => {
            world
                .resource_mut::<UiTree>()
                .set_prompt(menu.rebind, REBIND_PROMPT);
        }
        _ => {}
    }
    false
}

/// Whether `binding` is a pad's: a button or a stick direction.
fn is_pad(binding: Binding) -> bool {
    matches!(
        binding,
        Binding::GamepadButton { .. } | Binding::GamepadAxis { .. }
    )
}

/// Replaces [`REBIND_ACTION`]'s bindings of `binding`'s kind (keys and
/// mouse, or pad) with `binding`, keeping the rest, and persists through
/// the user layer (`D-159`). Escape keeps the old binding (Q9).
fn rebind(world: &mut World, binding: Binding) {
    if binding
        == (Binding::Key {
            code: tungsten::core::KeyCode::Escape,
        })
    {
        return;
    }
    let Some(map) = world.get_resource_mut::<ActionMap>() else {
        return;
    };
    let pad = is_pad(binding);
    let mut bindings = vec![binding];
    bindings.extend(
        map.bindings(REBIND_ACTION)
            .iter()
            .filter(|kept| is_pad(**kept) != pad)
            .copied(),
    );
    map.replace_bindings(REBIND_ACTION, bindings);
    if map.user_layer_path().is_some()
        && let Err(error) = map.persist()
    {
        println!("bindings not saved: {error}");
    }
}

// ---- the menu fixtures ----

/// The menu fixtures' menu: a `World` resource.
#[derive(Debug)]
pub struct BudgetMenu {
    /// The root.
    pub root: WidgetId,
}

/// Item names for the menu's buttons: varied letters, so a first open
/// rasterizes a realistic set of glyphs.
const ITEMS: [&str; 46] = [
    "Longsword",
    "Quartz Ring",
    "Healing Draught",
    "Iron Buckler",
    "Lantern",
    "Map of Vexley",
    "Jade Amulet",
    "Rope (50 ft)",
    "Flint & Steel",
    "Bedroll",
    "Wizard's Hat",
    "Cloak of Mist",
    "Throwing Axe",
    "Spyglass",
    "Bag of Holding",
    "Zephyr Boots",
    "Kettle",
    "Quill & Ink",
    "Obsidian Dagger",
    "Warhammer",
    "Elixir No. 9",
    "Yew Longbow",
    "Arrows x24",
    "Gauntlets",
    "Tome of Frost",
    "Crowbar",
    "Hourglass",
    "Lockpicks",
    "Mirror Shard",
    "Pickaxe",
    "Silver Key",
    "Torch",
    "Waterskin",
    "Xylophone",
    "Brass Compass",
    "Dice Set",
    "Fishing Rod",
    "Grappling Hook",
    "Herb Pouch",
    "Ivory Comb",
    "Jester's Bell",
    "Knight's Crest",
    "Moonstone",
    "Nightshade",
    "Ox Horn",
    "Prism",
];

/// Builds the menu fixtures' 100-widget menu into `tree`: a centred root
/// holding a title, a row of four columns of 46 buttons and a footer. With
/// each button's label, the tree holds 1 + 1 + 1 + 4 + 92 + 1 = 100 nodes.
/// Hidden when `hidden`.
pub fn build_budget_menu(tree: &mut UiTree, hidden: bool) -> BudgetMenu {
    let mut style = LayoutStyle::column()
        .with_padding(16.0)
        .with_gap(10.0)
        .anchored(Anchor::Center, Vec2::ZERO);
    style.align_items = Align::Center;
    let root = tree.add_root(UiLayer::Game, style);
    heading(tree, root, "Inventory — 46 items", 26.0);
    let row = tree.panel(root, LayoutStyle::row().with_gap(10.0));
    tree.set_overrides(row, transparent());
    let columns: Vec<WidgetId> = (0..4)
        .map(|_| {
            let column = tree.panel(row, LayoutStyle::column().with_gap(4.0));
            tree.set_overrides(column, transparent());
            column
        })
        .collect();
    for (i, item) in ITEMS.iter().enumerate() {
        button(tree, columns[i % columns.len()], item);
    }
    tree.label(root, "Quick-sort: weight, value, rarity — 9 kg free");
    if hidden {
        tree.set_visibility(root, tungsten::core::ui::Visibility::Hidden);
    }
    BudgetMenu { root }
}

/// The frames at which the `menu-prewarm` and `menu-cold` fixtures show,
/// hide and show the menu again.
pub const MENU_SHOW: u64 = 240;
/// See [`MENU_SHOW`].
pub const MENU_HIDE: u64 = 300;
/// See [`MENU_SHOW`].
pub const MENU_REOPEN: u64 = 360;

/// Counts frames for the menu fixtures; a `World` resource.
#[derive(Debug, Default)]
pub struct FixtureClock {
    /// Frames counted, the current one included.
    pub frame: u64,
}

/// Shows the hidden menu at frame [`MENU_SHOW`], hides it at
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

/// The `menus` fixture's script, by frame at smoke's pinned dt: pause at
/// frame 10, settings at 40, a capture started at 60 and cancelled at 70,
/// Back at 80 and Resume at 110, so the run is back in play well before
/// frame 150.
pub fn menus_script_system(world: &mut World) {
    let Some(clock) = world.get_resource_mut::<FixtureClock>() else {
        return;
    };
    clock.frame += 1;
    let frame = clock.frame;
    match frame {
        10 => {
            stack(world).request_push_transition(PauseState, fade());
        }
        40 => {
            stack(world).request_push_transition(SettingsState, fade());
        }
        60 => {
            if let Some(rebind) = world.get_resource::<SettingsMenu>().map(|m| m.rebind) {
                world.resource_mut::<UiRouter>().capture_next_input(rebind);
            }
        }
        70 => world.resource_mut::<UiRouter>().cancel_capture(),
        80 | 110 => {
            stack(world).request_pop_transition(fade());
        }
        _ => return,
    }
    let top = world.resource::<StateStack>().active_id().unwrap_or("");
    println!("ui_lab menus: frame {frame}, top '{top}'");
}
