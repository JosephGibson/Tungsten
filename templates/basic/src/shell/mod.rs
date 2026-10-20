//! The shell: the screens every game keeps, copied unchanged into a new
//! game. A title, pause, game over, save and load, tabbed settings,
//! keybindings, a confirm dialog and toasts, around a run the game's
//! gameplay state plays. It calls the game through six names in
//! `game.rs` and `save.rs` (the seam) and nothing else outside this folder.
//!
//! A run starts with [`start_run`] and ends at the title. Saves go to four
//! slots of `SaveSlots`: the autosave, written when the game pauses, when it
//! quits to the title or exits and on a run's first frame, and three the
//! player saves to. A slot that fails to load (a newer build's, or a damaged
//! file) is never loaded or overwritten for the rest of the run of the
//! program. Screens act only when no transition runs; a change of run goes
//! through [`goto`], which `shell_flow` applies after the dispatcher.

pub mod confirm;
pub mod game_over;
pub mod keybindings;
pub mod options;
pub mod pause;
pub mod saves;
pub mod settings;
pub mod theme;
pub mod title;
pub mod toast;
pub mod widgets;

use serde::{Deserialize, Serialize};
use tungsten::core::save::SaveError;
use tungsten::core::ui::{UiEvent, UiRouter, UiTree, WidgetId};
use tungsten::core::{
    ActionMap, EventQueue, InputState, RoutedInput, SaveSlots, Schedule, Settings, Stage, Time,
    World, system,
};
use tungsten::{DebugHud, Exiting, GameState, HudCorner, Plugin, RootCover, StateId, StateStack};

use crate::game;
use crate::save::{self, SaveGame};

/// The autosave's slot name.
pub const AUTOSAVE: &str = "autosave";
/// Every slot the shell reads, the autosave first.
pub const SLOTS: [&str; 4] = [AUTOSAVE, "slot_1", "slot_2", "slot_3"];

/// What the title shows: the window's title and the game's version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameInfo {
    /// The game's name, from `tungsten.json`'s `window.title`.
    pub title: String,
    /// The game's version, `CARGO_PKG_VERSION`.
    pub version: String,
}

/// How hard a run is; the game decides what it changes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Difficulty {
    Easy,
    #[default]
    Normal,
    Hard,
}

impl Difficulty {
    /// Every difficulty, easiest first.
    pub const ALL: [Self; 3] = [Self::Easy, Self::Normal, Self::Hard];

    /// Its name on screen.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Easy => "Easy",
            Self::Normal => "Normal",
            Self::Hard => "Hard",
        }
    }
}

/// What every save holds besides the game's own fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunMeta {
    /// The version of the game that wrote it.
    pub game_version: String,
    pub difficulty: Difficulty,
    /// Game-clock seconds played in the run.
    pub playtime_secs: f64,
}

/// How a run starts: its difficulty and, to continue one, its save.
#[derive(Debug, Clone, Default)]
pub struct RunStart {
    pub difficulty: Difficulty,
    pub save: Option<SaveGame>,
}

impl From<SaveGame> for RunStart {
    /// A run continued from `save`, at the save's difficulty.
    fn from(save: SaveGame) -> Self {
        let difficulty = save.meta.difficulty;
        let save = Some(save);
        Self { difficulty, save }
    }
}

/// The live run: a `World` resource from [`start_run`] to the title.
#[derive(Debug, Clone)]
pub struct RunState {
    /// The difficulty the run's next start takes, Retry included.
    pub difficulty: Difficulty,
    /// Game-clock seconds played, paused time excluded.
    pub playtime_secs: f64,
    /// Set by a save that succeeded, cleared by the next fixed step: a load
    /// from pause loses nothing then, so it does not ask.
    pub checkpointed: bool,
    /// The slot this run last loaded or saved successfully, which Retry
    /// loads; `None` starts Retry fresh.
    pub retry_from: Option<&'static str>,
    /// The run's first-frame autosave is due.
    first_save: bool,
}

/// One slot as the shell reads it.
#[derive(Debug, Clone)]
pub enum SlotState {
    /// No user folder: saving is off.
    Off,
    Empty,
    /// A save this build reads, with when it was written (Unix ms).
    Ready(Box<SaveGame>, u64),
    /// Written by a newer version of the game.
    Newer,
    Damaged,
}

/// A change of run [`goto`] asked for, which `shell_flow` applies.
struct Flow {
    replace: Box<dyn FnOnce(&mut StateStack)>,
    run_start: bool,
}

/// The shell's own state: a `World` resource.
#[derive(Default)]
pub(crate) struct Shell {
    unfocused: bool,
    /// The window lost focus with "Mute when unfocused" on.
    focus_muted: bool,
    exit_saved: bool,
    pub(crate) warned: bool,
    flow: Option<Flow>,
    /// Slots that failed to load: never loaded or written this run.
    pub(crate) read_only: Vec<&'static str>,
}

/// The shell as a plugin: the title first, the run's systems, the options
/// store and the engine's HUD at the top right, hidden until F4 shows it.
pub struct ShellPlugin {
    info: GameInfo,
}

impl ShellPlugin {
    #[must_use]
    pub fn new(info: GameInfo) -> Self {
        Self { info }
    }
}

impl Plugin for ShellPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        world.insert_resource(self.info.clone());
        world.insert_resource(Shell::default());
        world.insert_resource(toast::Toasts::default());
        options::load(world);
        if let Some(hud) = world.get_resource_mut::<DebugHud>() {
            hud.enabled = false;
            hud.corner = HudCorner::TopRight;
        }
        schedule.add(Stage::Startup, system("shell_start", start));
        // `shell_frame` first, so a run's first-frame autosave waits for the
        // frame after `shell_flow` queued the run's state.
        schedule.add(Stage::Update, system("shell_frame", frame));
        schedule.add(Stage::Update, system("shell_flow", flow));
        schedule.add(Stage::FixedUpdate, system("shell_checkpoint", checkpoint));
    }
}

/// The startup system: the options applied, then the title. The harness runs
/// `Startup` on its first step.
pub fn start(world: &mut World) {
    options::apply(world);
    stack(world).request_push(title::TitleState::default());
}

/// The live run, if any.
#[must_use]
pub fn run(world: &World) -> Option<&RunState> {
    world.get_resource::<RunState>()
}

/// The run's part of a save; `None` with no live run.
#[must_use]
pub fn run_meta(world: &World) -> Option<RunMeta> {
    let run = run(world)?;
    Some(RunMeta {
        game_version: world.resource::<GameInfo>().version.clone(),
        difficulty: run.difficulty,
        playtime_secs: run.playtime_secs,
    })
}

/// Starts a run: the game's gameplay state replaces the whole stack.
/// `from` names the slot the save came from, which Retry then loads.
pub fn start_run(world: &mut World, start: RunStart, from: Option<&'static str>) {
    let playtime_secs = start
        .save
        .as_ref()
        .map_or(0.0, |save| save.meta.playtime_secs);
    world.insert_resource(RunState {
        difficulty: start.difficulty,
        playtime_secs,
        checkpointed: false,
        retry_from: from,
        first_save: false,
    });
    set_flow(world, game::gameplay(start), true);
}

/// Replaces the whole stack with `state`, after the dispatcher: a screen's
/// `update` runs off the stack, so it cannot count the states itself.
pub fn goto(world: &mut World, state: impl GameState) {
    set_flow(world, state, false);
}

fn set_flow(world: &mut World, state: impl GameState, run_start: bool) {
    let replace = Box::new(move |stack: &mut StateStack| stack.request_replace(state));
    world.resource_mut::<Shell>().flow = Some(Flow { replace, run_start });
}

/// Whether a [`goto`] waits for `shell_flow`.
#[must_use]
pub fn flow_pending(world: &World) -> bool {
    world.resource::<Shell>().flow.is_some()
}

/// `shell_flow`: a plain pop for each state above the bottom one, then a
/// replace of the bottom one, so loading from [gameplay, pause, saves]
/// leaves one gameplay.
fn flow(world: &mut World) {
    let Some(flow) = world.resource_mut::<Shell>().flow.take() else {
        return;
    };
    let stack = stack(world);
    for _ in 1..stack.depth() {
        stack.request_pop();
    }
    (flow.replace)(stack);
    if let Some(run) = world.get_resource_mut::<RunState>() {
        run.first_save |= flow.run_start;
    }
}

/// Pushes `state` over the screen on top, behind a fade.
pub fn push(world: &mut World, state: impl GameState) {
    stack(world).request_push_transition(state, theme::fade());
}

/// Pops the screen on top, behind a fade.
pub fn pop(world: &mut World) {
    stack(world).request_pop_transition(theme::fade());
}

/// Opens the game-over screen over gameplay: the sample calls it from its
/// gameplay state's `update`, which runs off the stack, when its player dies.
pub fn game_over(world: &mut World) {
    if !transitioning(world) && matches!(top(world), None | Some(game::GAMEPLAY)) {
        push(world, game_over::GameOverState::default());
    }
}

/// Applies `Settings` through the engine, keeping the focus mute: while it
/// holds, the engine sees a copy at volume 0, so no other master volume is
/// queued.
pub fn apply_settings(world: &mut World) {
    if !world.resource::<Shell>().focus_muted {
        return tungsten::apply_settings(world);
    }
    let saved = world.resource::<Settings>().clone();
    let mut quiet = saved.clone();
    quiet.set_master_volume(Some(0.0));
    world.insert_resource(quiet);
    tungsten::apply_settings(world);
    world.insert_resource(saved);
}

/// Whether the game has a user folder to save in.
#[must_use]
pub fn saving_on(world: &World) -> bool {
    world.resource::<SaveSlots>().dir().is_some()
}

/// Reads every slot of [`SLOTS`], in order, marking the ones that fail to
/// load read-only; with how many files in the folder could not be read.
pub fn read_slots(world: &mut World) -> (Vec<(&'static str, SlotState)>, usize) {
    if !saving_on(world) {
        return (
            SLOTS.iter().map(|slot| (*slot, SlotState::Off)).collect(),
            0,
        );
    }
    let slots = world.resource::<SaveSlots>();
    let list = slots.list().unwrap_or_default();
    let read = |slot: &'static str| {
        let saved = list.slots.iter().find(|info| info.name == slot);
        let state = match slots.load::<SaveGame>(slot) {
            Ok(save) => SlotState::Ready(Box::new(save), saved.map_or(0, |info| info.saved_at_ms)),
            Err(SaveError::NotFound { .. }) => SlotState::Empty,
            Err(SaveError::NewerSchema { .. } | SaveError::UnknownEnvelope { .. }) => {
                SlotState::Newer
            }
            Err(error) => {
                log::warn!("Save slot '{slot}' not loaded: {error}");
                SlotState::Damaged
            }
        };
        (slot, state)
    };
    let scan: Vec<_> = SLOTS.into_iter().map(read).collect();
    let shell = world.resource_mut::<Shell>();
    for (slot, state) in &scan {
        let failed = matches!(state, SlotState::Newer | SlotState::Damaged);
        if failed && !shell.read_only.contains(slot) {
            shell.read_only.push(slot);
        }
    }
    (scan, list.warnings.len())
}

/// The newest readable slot of `slots`, by its save time.
#[must_use]
pub fn newest(slots: &[(&'static str, SlotState)]) -> Option<(&'static str, SaveGame)> {
    let ready = slots.iter().filter_map(|(slot, state)| match state {
        SlotState::Ready(save, at) => Some((*at, *slot, save)),
        _ => None,
    });
    let (_, slot, save) = ready.max_by_key(|(at, ..)| *at)?;
    Some((slot, save.as_ref().clone()))
}

/// Saves the live run to `slot`; `false` when there is nothing to save,
/// no folder, a read-only slot or a failed write, which keeps the old file.
pub fn save_to(world: &mut World, slot: &'static str) -> bool {
    let Some(save) = save::capture(world) else {
        return false;
    };
    if world.resource::<Shell>().read_only.contains(&slot) {
        return false;
    }
    match world.resource::<SaveSlots>().save(slot, &save) {
        Ok(()) => {
            let run = world.resource_mut::<RunState>();
            run.checkpointed = true;
            run.retry_from = Some(slot);
            true
        }
        Err(SaveError::NoFolder) => false,
        Err(error) => {
            log::warn!("Save slot '{slot}' not saved: {error}");
            false
        }
    }
}

/// [`save_to`] the autosave.
pub fn autosave(world: &mut World) -> bool {
    save_to(world, AUTOSAVE)
}

/// Quits to the title, autosaving first; asks when a live run cannot be
/// autosaved.
pub fn quit_to_title(world: &mut World) {
    if save::capture(world).is_none() || autosave(world) {
        return goto(world, title::TitleState::default());
    }
    let question = "Quit to the title? Progress since your last save will be lost.";
    let ask = confirm::ConfirmState::new(question, "Quit", "Stay");
    confirm::ask(
        world,
        ask.on_yes(|world| goto(world, title::TitleState::default())),
    );
}

/// A menu screen a state owns: a modal root with a heading, "Saving is off"
/// without a user folder, and a column of buttons, sliding in.
pub struct Menu {
    pub root: WidgetId,
    pub buttons: Vec<WidgetId>,
    /// The button that opened the screen above, which takes focus back.
    opened: Option<WidgetId>,
}

impl Menu {
    pub fn new(world: &mut World, heading: &str, captions: &[&str]) -> Self {
        let saving = saving_on(world);
        let tree = world.resource_mut::<UiTree>();
        let root = widgets::screen(tree, theme::MENU_WIDTH);
        widgets::heading(tree, root, heading);
        if !saving {
            widgets::note(tree, root, "Saving is off");
        }
        let buttons = captions
            .iter()
            .map(|caption| widgets::button(tree, root, caption));
        let buttons = buttons.collect();
        theme::slide_in(tree, root);
        stack(world).own_root(root, RootCover::Hide);
        let opened = None;
        let menu = Self {
            root,
            buttons,
            opened,
        };
        menu.focus(world);
        menu
    }

    /// Focuses the button that opened the screen above, else the first that
    /// can take focus.
    pub fn focus(&self, world: &mut World) {
        let candidates = self.opened.into_iter().chain(self.buttons.iter().copied());
        widgets::focus_first(world.resource_mut::<UiTree>(), candidates);
    }

    /// The button activated at this routing, which then takes focus back
    /// when the screen above closes.
    pub fn pick(&mut self, world: &World) -> Option<usize> {
        let at = clicked(world, &self.buttons)?;
        self.opened = Some(self.buttons[at]);
        Some(at)
    }
}

/// `shell_frame`: the run's playtime and first autosave, the exit's save,
/// mute, the pause key, focus loss and the toasts.
fn frame(world: &mut World) {
    let delta = f64::from(world.resource::<Time>().delta());
    let first_save = world.get_resource_mut::<RunState>().is_some_and(|run| {
        run.playtime_secs += delta;
        std::mem::take(&mut run.first_save)
    });
    let exit_save = world.has_resource::<Exiting>()
        && !std::mem::replace(&mut world.resource_mut::<Shell>().exit_saved, true);
    if first_save || exit_save {
        autosave(world);
    }
    if !capturing(world) && pressed(world, "toggle_mute", false) {
        toggle_mute(world);
    }
    let open = idle(world) && top(world) == Some(game::GAMEPLAY);
    if open && pressed(world, "state_pause", true) {
        push(world, pause::PauseState::default());
    }
    focus(world, open);
    toast::tick(world);
}

/// Mutes or unmutes and keeps the choice in `Settings`: the master volume
/// set to 0, or unset to return to full.
fn toggle_mute(world: &mut World) {
    let settings = world.resource_mut::<Settings>();
    let muted = settings.master_volume().is_some_and(|volume| volume <= 0.0);
    settings.set_master_volume(if muted { None } else { Some(0.0) });
    apply_settings(world);
    if let Err(error) = world.resource::<Settings>().save() {
        log::warn!("Settings not saved: {error}");
    }
}

/// Focus loss: mutes when the options say so, and pauses gameplay when
/// `open` (gameplay on top, no transition); focus back unmutes.
fn focus(world: &mut World, open: bool) {
    let unfocused = !world.resource::<InputState>().focused();
    let shell = world.resource_mut::<Shell>();
    if std::mem::replace(&mut shell.unfocused, unfocused) == unfocused {
        return;
    }
    let options = *world.resource::<options::GameOptions>();
    let muted = unfocused && options.mute_unfocused;
    if std::mem::replace(&mut world.resource_mut::<Shell>().focus_muted, muted) != muted {
        apply_settings(world);
    }
    if unfocused && open && options.pause_unfocused {
        push(world, pause::PauseState::default());
    }
}

/// `shell_checkpoint`: a fixed step ran, so the run has moved past its last
/// save.
fn checkpoint(world: &mut World) {
    if let Some(run) = world.get_resource_mut::<RunState>() {
        run.checkpointed = false;
    }
}

/// The state stack.
pub fn stack(world: &mut World) -> &mut StateStack {
    world.resource_mut::<StateStack>()
}

/// The state on top.
#[must_use]
pub fn top(world: &World) -> Option<StateId> {
    world.resource::<StateStack>().active_id()
}

/// Whether a screen transition is queued or running.
#[must_use]
pub fn transitioning(world: &World) -> bool {
    world.resource::<StateStack>().is_transitioning()
}

/// Whether a widget is capturing the next press for a rebind.
#[must_use]
pub fn capturing(world: &World) -> bool {
    world.resource::<UiRouter>().capturing().is_some()
}

/// Whether a screen may act: no transition, run change or capture waits.
#[must_use]
pub fn idle(world: &World) -> bool {
    !transitioning(world) && !flow_pending(world) && !capturing(world)
}

/// The UI's events at this routing.
#[must_use]
pub fn events(world: &World) -> Vec<UiEvent> {
    let events = world.resource::<EventQueue<UiEvent>>();
    events.iter_current().copied().collect()
}

/// The first of `buttons` activated at this routing.
#[must_use]
pub fn clicked(world: &World, buttons: &[WidgetId]) -> Option<usize> {
    let events = world.resource::<EventQueue<UiEvent>>();
    buttons.iter().position(|button| events.activated(*button))
}

/// Whether `ui_cancel` backed out of `root` at this routing.
#[must_use]
pub fn cancelled(world: &World, root: WidgetId) -> bool {
    let mut events = world.resource::<EventQueue<UiEvent>>().iter_current();
    events.any(|event| matches!(event, UiEvent::Cancel { scope, .. } if *scope == root))
}

/// `action` just pressed: in what the UI left to the game when `routed`,
/// else in the raw input, which a modal screen reads for its own hotkeys.
#[must_use]
pub fn pressed(world: &World, action: &str, routed: bool) -> bool {
    let input: &InputState = match routed {
        true => world.resource::<RoutedInput>(),
        false => world.resource::<InputState>(),
    };
    world.resource::<ActionMap>().just_pressed(input, action)
}
