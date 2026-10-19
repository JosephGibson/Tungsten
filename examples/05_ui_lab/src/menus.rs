//! State-owned screens (`D-163`): a `play` state owning the game screen's
//! roots and its HUD, a `pause` state's modal menu and a `settings` state's
//! screen, each pushed and popped behind a fade and sliding in. A state
//! owns its roots on the state stack, so a covered screen hides (`Hide`)
//! or stays drawn and frozen (`Show`), and a transition freezes and fades
//! every owned root with the frame. A screen reads its widgets' events
//! from the routing in its `update`, and acts only when no transition runs.
//!
//! The settings screen shows the runtime apply (`D-159`, `D-169`): each
//! change goes into `Settings` and `apply_settings` applies it as a restart
//! would; Back or `ui_cancel` saves the file and pops. Its music and effects
//! sliders set the buses' volumes, and each effects change plays a blip at
//! the new level. Its rebind row captures the next key, button or stick
//! direction (`D-164`) and persists the action map's user layer. The pad's
//! Start pauses through a binding added in code ([`add_pause_pad_binding`]).
//!
//! The play state's music fades in over a second on entry and out over half
//! a second on exit, and plays on through the pause menu and the
//! inventory; [`MUSIC_ACTION`] (M, which [`add_music_binding`] adds when the
//! map lacks it) crossfades it into a new voice from its start over two
//! seconds.

use tungsten::core::ui::{
    LayoutStyle, Sizing, SliderRange, UiEvent, UiRouter, UiTree, UiValue, WidgetId,
};
use tungsten::core::{
    ActionMap, AssetRegistry, AudioBus, AudioCommands, Binding, DisplayMode, DisplayState,
    EventQueue, GamepadButton, KeyCode, RoutedInput, Settings, SoundRegistry, Time, Voice, World,
};
use tungsten::{GameState, RootCover, StateContext, StateId, StateStack, apply_settings};

use crate::controls;
use crate::hud::{self, PlayHud};
use crate::inventory::{self, InventoryState};
use crate::theme;
use crate::widgets;

/// The action the play state pauses on: P in the engine defaults, and the
/// pad's Start, which [`add_pause_pad_binding`] adds in code.
pub const PAUSE_ACTION: &str = "state_pause";
/// The action the settings screen rebinds.
pub const REBIND_ACTION: &str = "jump";
/// The action that crossfades the play state's music: M in the root
/// `input.json`, which [`add_music_binding`] adds when the map lacks it.
pub const MUSIC_ACTION: &str = "audio_toggle_music";
/// The play state's music, on the music bus.
pub const MUSIC_SOUND: &str = "music_main";
/// The blip an effects volume change plays, on the effects bus.
pub const BLIP_SOUND: &str = "sfx_blip";
/// Seconds the music takes to rise on entering play.
pub const MUSIC_FADE_IN: f32 = 1.0;
/// Seconds the music takes to fall on leaving play.
pub const MUSIC_FADE_OUT: f32 = 0.5;
/// Seconds a crossfade into a new music voice takes.
pub const MUSIC_CROSSFADE: f32 = 2.0;

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

/// Adds M to [`MUSIC_ACTION`]'s bindings when the map has none of it, so a
/// map built without the root `input.json` (the harness's) has it too.
pub fn add_music_binding(world: &mut World) {
    let Some(map) = world.get_resource_mut::<ActionMap>() else {
        return;
    };
    let key = Binding::Key {
        code: KeyCode::KeyM,
    };
    let mut bindings = map.bindings(MUSIC_ACTION).to_vec();
    if !bindings.contains(&key) {
        bindings.push(key);
        map.replace_bindings(MUSIC_ACTION, bindings);
    }
}

/// Plays `id` at its manifest volume, looping as asked, rising over
/// `fade`; `None` when the manifest has no such sound.
fn play_sound(world: &mut World, id: &str, looping: bool, fade: f32) -> Option<Voice> {
    let (handle, volume) = {
        let sounds = world.get_resource::<SoundRegistry>()?;
        let handle = sounds.get_by_id(id)?;
        (handle, sounds.get_volume(handle))
    };
    let audio = world.get_resource_mut::<AudioCommands>()?;
    Some(audio.play_faded(handle, volume, looping, fade))
}

/// Stops `voice` over `fade` seconds.
fn stop_voice(world: &mut World, voice: Voice, fade: f32) {
    if let Some(audio) = world.get_resource_mut::<AudioCommands>() {
        audio.stop_voice(voice, fade);
    }
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

pub(crate) fn stack(world: &mut World) -> &mut StateStack {
    world.resource_mut::<StateStack>()
}

// ---- play ----

/// Applies the lab's theme and builds the play screen's roots: the lab HUD
/// (`Show`) and, when `controls` is set, the controls window (`Hide`), each
/// inserted as its resource; returns them with their covers for
/// [`PlayState::new`]. The play HUD comes with the state.
pub fn build_play_screen(world: &mut World, controls: bool) -> Vec<(WidgetId, RootCover)> {
    let Some(mut tree) = world.remove_resource::<UiTree>() else {
        return Vec::new();
    };
    theme::apply(world, &mut tree);
    let lab_hud = {
        let sprites = world.resource_mut::<AssetRegistry>();
        hud::build_lab_hud(&mut tree, sprites)
    };
    let mut roots = vec![(lab_hud.root, RootCover::Show)];
    if controls {
        let window = controls::build_controls(&mut tree);
        roots.push((window.window, RootCover::Hide));
        world.insert_resource(window);
    }
    world.insert_resource(tree);
    world.insert_resource(lab_hud);
    roots
}

/// The game screen: owns the lab's roots, each with its cover (the lab
/// HUD `Show`, so it stays drawn and frozen under a pushed screen; the
/// controls window `Hide`, so no pushed screen shares the screen with it),
/// and the play HUD (`Show`); counts game time; pushes [`PauseState`] on
/// [`PAUSE_ACTION`] and [`InventoryState`] on `inventory_open`, each
/// behind a fade. Pauses the game clock while covered. Plays the music
/// from entry to exit and crossfades it on [`MUSIC_ACTION`].
pub struct PlayState {
    roots: Vec<(WidgetId, RootCover)>,
    /// The music's voice while it plays.
    music: Option<Voice>,
}

impl PlayState {
    /// A play state that will own `roots`, the lab's, with their covers.
    #[must_use]
    pub fn new(roots: Vec<(WidgetId, RootCover)>) -> Self {
        Self { roots, music: None }
    }
}

impl GameState for PlayState {
    fn id(&self) -> StateId {
        "play"
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        let play_hud = hud::build_play_hud(ctx.world.resource_mut::<UiTree>());
        let stack = stack(ctx.world);
        for (owned, cover) in self
            .roots
            .iter()
            .copied()
            .chain([(play_hud.root, RootCover::Show)])
        {
            stack.own_root(owned, cover);
        }
        ctx.world.insert_resource(play_hud);
        self.music = play_sound(ctx.world, MUSIC_SOUND, true, MUSIC_FADE_IN);
    }

    fn on_exit(&mut self, ctx: &mut StateContext) {
        ctx.world.remove_resource::<PlayHud>();
        if let Some(music) = self.music.take() {
            stop_voice(ctx.world, music, MUSIC_FADE_OUT);
        }
    }

    fn on_pause(&mut self, ctx: &mut StateContext) {
        ctx.world.resource_mut::<Time>().pause();
    }

    fn on_resume(&mut self, ctx: &mut StateContext) {
        ctx.world.resource_mut::<Time>().resume();
    }

    fn update(&mut self, world: &mut World) {
        hud::write_play_timer(world);
        let pressed = |action: &str| {
            world
                .get_resource::<ActionMap>()
                .zip(world.get_resource::<RoutedInput>())
                .is_some_and(|(map, routed)| map.just_pressed(routed, action))
        };
        let (pause, open) = (pressed(PAUSE_ACTION), pressed(inventory::OPEN_ACTION));
        let crossfade = pressed(MUSIC_ACTION);
        if world.resource::<StateStack>().is_transitioning() {
            return;
        }
        if crossfade && let Some(music) = self.music {
            stop_voice(world, music, MUSIC_CROSSFADE);
            self.music = play_sound(world, MUSIC_SOUND, true, MUSIC_CROSSFADE);
        }
        if pause {
            stack(world).request_push_transition(PauseState, theme::fade());
        } else if open {
            stack(world).request_push_transition(InventoryState, theme::fade());
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
    /// "Inventory".
    pub inventory: WidgetId,
    /// The prompt row.
    pub prompt: WidgetId,
    /// The button that pushed the screen above, which takes focus back.
    opened: Option<WidgetId>,
}

/// The pause screen: a modal "Paused" menu (Resume, Settings, Inventory)
/// with a prompt row, owned with `Hide`; Resume or `ui_cancel` pops it,
/// Settings pushes [`SettingsState`] and Inventory [`InventoryState`], each
/// behind a fade.
pub struct PauseState;

impl GameState for PauseState {
    fn id(&self) -> StateId {
        "pause"
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        let tree = ctx.world.resource_mut::<UiTree>();
        let root = widgets::screen_root(tree, PAUSE_WIDTH);
        widgets::heading(tree, root, "Paused");
        let resume = widgets::button(tree, root, "Resume");
        let settings = widgets::button(tree, root, "Settings");
        let inventory = widgets::button(tree, root, "Inventory");
        let prompt = widgets::prompt_row(
            tree,
            root,
            "{action:ui_accept} select   {action:ui_cancel} resume",
        );
        tree.focus(resume);
        theme::slide_in(tree, root);
        stack(ctx.world).own_root(root, RootCover::Hide);
        ctx.world.insert_resource(PauseMenu {
            root,
            resume,
            settings,
            inventory,
            prompt,
            opened: None,
        });
    }

    fn on_exit(&mut self, ctx: &mut StateContext) {
        ctx.world.remove_resource::<PauseMenu>();
    }

    fn on_resume(&mut self, ctx: &mut StateContext) {
        if let Some(button) = ctx
            .world
            .get_resource::<PauseMenu>()
            .map(|menu| menu.opened.unwrap_or(menu.settings))
        {
            ctx.world.resource_mut::<UiTree>().focus(button);
        }
    }

    fn update(&mut self, world: &mut World) {
        let Some(menu) = world.get_resource::<PauseMenu>() else {
            return;
        };
        let (root, resume, settings, inventory) =
            (menu.root, menu.resume, menu.settings, menu.inventory);
        if world.resource::<StateStack>().is_transitioning() {
            return;
        }
        if activated(world, resume) || cancelled(world, root) {
            stack(world).request_pop_transition(theme::fade());
        } else if activated(world, settings) {
            world.resource_mut::<PauseMenu>().opened = Some(settings);
            stack(world).request_push_transition(SettingsState, theme::fade());
        } else if activated(world, inventory) {
            world.resource_mut::<PauseMenu>().opened = Some(inventory);
            stack(world).request_push_transition(InventoryState, theme::fade());
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
    /// The music bus's volume, 0 to 100 by 5, applied live.
    pub music: WidgetId,
    /// The effects bus's volume, 0 to 100 by 5, applied live; each change
    /// plays a blip.
    pub effects: WidgetId,
    /// Shows [`REBIND_ACTION`]'s binding and captures a new one.
    pub rebind: WidgetId,
    /// Saves the settings and pops.
    pub back: WidgetId,
    /// The display requests the screen has made.
    pub display_requests: u32,
    /// The scale slider the pointer holds, and the value it would set.
    dragging: Option<(WidgetId, Option<f32>)>,
}

/// The settings screen: display mode, UI and text scale, the master, music
/// and effects volumes, one rebind and Back, which saves the settings and
/// pops behind a fade, as `ui_cancel` does.
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

const REBIND_PROMPT: &str = "{action:jump}";

/// The pause menu's width, in UI units.
const PAUSE_WIDTH: f32 = 240.0;

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
        let root = widgets::screen_root(tree, widgets::setting_width(0.0));
        widgets::heading(tree, root, "Settings");
        let display_mode = widgets::button(tree, root, mode_text(mode));
        let (_, ui_scale) = widgets::slider_row(
            tree,
            root,
            "UI scale",
            SliderRange::new(0.5, 2.0).with_step(0.25),
            settings.ui_scale().unwrap_or(1.0),
        );
        let (_, text_scale) = widgets::slider_row(
            tree,
            root,
            "Text scale",
            SliderRange::new(0.75, 2.0).with_step(0.25),
            settings.text_scale().unwrap_or(1.0),
        );
        let (_, volume) = widgets::slider_row(
            tree,
            root,
            "Volume",
            SliderRange::new(0.0, 100.0).with_step(5.0),
            settings.master_volume().unwrap_or(1.0) * 100.0,
        );
        let [music, effects] =
            [("Music", AudioBus::Music), ("Effects", AudioBus::Effects)].map(|(caption, bus)| {
                widgets::slider_row(
                    tree,
                    root,
                    caption,
                    SliderRange::new(0.0, 100.0).with_step(5.0),
                    settings.bus_volume(bus).unwrap_or(1.0) * 100.0,
                )
                .1
            });
        let (row, _) = widgets::caption_row(tree, root, "Jump");
        let rebind = widgets::button(tree, row, "");
        tree.set_layout_style(
            rebind,
            LayoutStyle {
                width: Sizing::Px(theme::CONTROL_WIDTH),
                ..widgets::button_style()
            },
        );
        tree.set_prompt(rebind, REBIND_PROMPT);
        let back = widgets::button(tree, root, "Back");
        tree.focus(display_mode);
        theme::slide_in(tree, root);
        stack(ctx.world).own_root(root, RootCover::Hide);
        ctx.world.insert_resource(SettingsMenu {
            root,
            display_mode,
            ui_scale,
            text_scale,
            volume,
            music,
            effects,
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
            stack(world).request_pop_transition(theme::fade());
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
            cycle_display_mode(world, menu);
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
        } if widget == menu.volume => set_volume(world, value),
        UiEvent::ValueChanged {
            widget,
            value: UiValue::Number(value),
            ..
        } if widget == menu.music => set_bus_volume(world, AudioBus::Music, value),
        UiEvent::ValueChanged {
            widget,
            value: UiValue::Number(value),
            ..
        } if widget == menu.effects => {
            set_bus_volume(world, AudioBus::Effects, value);
            play_sound(world, BLIP_SOUND, false, 0.0);
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

/// The display button's action: the mode after the current one of
/// windowed, borderless and exclusive, set in `Settings` and applied, and
/// the button's text. The `display` fixture runs it too.
pub fn cycle_display_mode(world: &mut World, menu: &mut SettingsMenu) {
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

/// The volume slider's action: the master volume at `percent`, applied.
pub fn set_volume(world: &mut World, percent: f32) {
    change_settings(world, |settings| {
        settings.set_master_volume(Some(percent / 100.0));
    });
}

/// A bus slider's action: `bus`'s volume at `percent`, applied.
pub fn set_bus_volume(world: &mut World, bus: AudioBus, percent: f32) {
    change_settings(world, |settings| {
        settings.set_bus_volume(bus, Some(percent / 100.0));
    });
}

/// One step of the volume slider, as an arrow key moves it: 5 down, or up
/// from 0, then [`set_volume`]. The `display` fixture runs it.
pub fn step_volume(world: &mut World, menu: &SettingsMenu) {
    let tree = world.resource_mut::<UiTree>();
    let value = tree.slider_value(menu.volume).unwrap_or(100.0);
    let next = if value >= 5.0 {
        value - 5.0
    } else {
        value + 5.0
    };
    tree.set_slider_value(menu.volume, next);
    set_volume(world, next);
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
