//! The shell: the screens every game keeps, copied unchanged into a new
//! game. A title, pause, game over, save and load, tabbed settings,
//! keybindings, a confirm dialog and toasts, around a run the game's
//! gameplay state plays. It calls the game through seven names in
//! `game.rs` and `save.rs` (the seam) and nothing else outside this folder.
//!
//! A run starts with [`start_run`] and ends at the title. Saves go to eleven
//! slots of `SaveSlots`: the autosave, written when the game pauses, when it
//! quits to the title or exits and on a run's first frame, and ten the
//! player saves to. A slot that fails to load (a newer build's, or a damaged
//! file) is never loaded or overwritten for the rest of the run of the
//! program. One screen change runs at a time, and screens act only when
//! none runs: a push acts at once, a pop after its screen fades out, and a
//! change of run, through [`goto`] or [`start_run`], behind a curtain.

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
use tungsten::core::ui::{UiEvent, UiRouter, UiTrack, UiTree, Visibility, WidgetId};
use tungsten::core::{
    ActionMap, AudioCommands, EventQueue, InputState, RoutedInput, SaveSlots, ScaleFactor,
    Schedule, Settings, SoundRegistry, Stage, Time, Voice, World, system,
};
use tungsten::{
    DebugHud, Exiting, GameState, HudCorner, Plugin, RootCover, StateId, StateStack, request_exit,
};

use crate::game;
use crate::save::{self, SaveGame};

/// The autosave's slot name.
pub const AUTOSAVE: &str = "autosave";
/// Every slot the shell reads, the autosave first.
pub const SLOTS: [&str; 11] = [
    AUTOSAVE, "slot_1", "slot_2", "slot_3", "slot_4", "slot_5", "slot_6", "slot_7", "slot_8",
    "slot_9", "slot_10",
];

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

/// A change of run [`goto`] asked for, which `shell_flow` applies under
/// the curtain: the replace, and the incoming run's state, staged so a save
/// before the swap holds the old run whole.
struct Flow {
    replace: Box<dyn FnOnce(&mut StateStack)>,
    run: Option<RunState>,
}

/// The screen change under way; one runs at a time.
enum Change {
    /// A push or pop asked for at this frame, which the dispatcher applies
    /// at the next.
    Queued(u64),
    /// The top screen fading out, popped once its seconds run out.
    Closing(f32),
    /// A change of run: the curtain, its seconds so far and the flow, which
    /// applies once the curtain is opaque.
    Curtain(WidgetId, f32, Option<Flow>),
}

/// The shell's own state: a `World` resource.
#[derive(Default)]
pub(crate) struct Shell {
    unfocused: bool,
    /// The window lost focus with "Mute when unfocused" on.
    focus_muted: bool,
    exit_saved: bool,
    pub(crate) warned: bool,
    change: Option<Change>,
    /// Slots that failed to load: never loaded or written this run.
    pub(crate) read_only: Vec<&'static str>,
}

/// The shell as a plugin: the title first, the run's systems, the options
/// store, the engine's HUD at the top right, hidden until F4 shows it, and
/// the UI sized from the window's height under a floor (`D-184`).
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
        if let Some(scale) = world.get_resource_mut::<ScaleFactor>() {
            scale.set_reference_height(Some(theme::REFERENCE_HEIGHT));
            scale.set_min_viewport(Some(theme::MIN_VIEWPORT.into()));
        }
        if let Some(hud) = world.get_resource_mut::<DebugHud>() {
            hud.enabled = false;
            hud.corner = HudCorner::TopRight;
        }
        schedule.add(Stage::Startup, system("shell_start", start));
        // `shell_frame` first, so a run's first-frame autosave waits for the
        // frame after `shell_flow` swapped the run in.
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

/// Starts a run behind the curtain: the game's gameplay state replaces the
/// whole stack, and its run state the old one, both at the swap. `from`
/// names the slot the save came from, which Retry then loads.
pub fn start_run(world: &mut World, start: RunStart, from: Option<&'static str>) {
    let playtime_secs = start
        .save
        .as_ref()
        .map_or(0.0, |save| save.meta.playtime_secs);
    let run = RunState {
        difficulty: start.difficulty,
        playtime_secs,
        checkpointed: false,
        retry_from: from,
        first_save: true,
    };
    set_flow(world, game::gameplay(start), Some(run));
}

/// Replaces the whole stack with `state` behind the curtain: a screen's
/// `update` runs off the stack, so it cannot count the states itself.
pub fn goto(world: &mut World, state: impl GameState) {
    set_flow(world, state, None);
}

fn set_flow(world: &mut World, state: impl GameState, run: Option<RunState>) {
    if transitioning(world) {
        return;
    }
    let replace = Box::new(move |stack: &mut StateStack| stack.request_replace(state));
    let tree = world.resource_mut::<UiTree>();
    let curtain = widgets::curtain(tree);
    tree.animate(curtain, UiTrack::opacity(0.0, 1.0, theme::CURTAIN_IN_SECS));
    let flow = Some(Flow { replace, run });
    world.resource_mut::<Shell>().change = Some(Change::Curtain(curtain, 0.0, flow));
}

/// Whether a [`goto`] waits for the curtain.
#[must_use]
pub fn flow_pending(world: &World) -> bool {
    let change = &world.resource::<Shell>().change;
    matches!(change, Some(Change::Curtain(_, _, Some(_))))
}

/// `shell_flow`: the closing screen's pop and the curtain's swap once their
/// time comes, then the toasts, which stay above the curtain.
fn flow(world: &mut World) {
    let time = world.resource::<Time>();
    let (dt, frame) = (time.real_delta(), time.frame());
    let shell = world.resource_mut::<Shell>();
    match shell.change.take() {
        Some(Change::Closing(secs)) if secs > dt => shell.change = Some(Change::Closing(secs - dt)),
        Some(Change::Closing(_)) => {
            shell.change = Some(Change::Queued(frame));
            stack(world).request_pop();
        }
        Some(Change::Curtain(root, secs, flow)) => curtain(world, root, secs + dt, flow),
        change => shell.change = change,
    }
    toast::tick(world);
}

/// The curtain at `secs`: once opaque, a plain pop for each state above the
/// bottom one, a replace of the bottom one (so loading from [gameplay,
/// pause, saves] leaves one gameplay) and the staged run; gone once faded.
fn curtain(world: &mut World, root: WidgetId, secs: f32, mut flow: Option<Flow>) {
    let tree = world.resource_mut::<UiTree>();
    if secs >= theme::CURTAIN_IN_SECS + theme::CURTAIN_OUT_SECS {
        return tree.remove(root);
    }
    tree.raise(root);
    if secs >= theme::CURTAIN_IN_SECS
        && let Some(Flow { replace, run }) = flow.take()
    {
        tree.animate(root, UiTrack::opacity(1.0, 0.0, theme::CURTAIN_OUT_SECS));
        let stack = stack(world);
        for _ in 1..stack.depth() {
            stack.request_pop();
        }
        replace(stack);
        if let Some(run) = run {
            world.insert_resource(run);
        }
    }
    world.resource_mut::<Shell>().change = Some(Change::Curtain(root, secs, flow));
}

/// Pushes `state` over the screen on top, which stays drawn under its
/// scrim; nothing while another change runs.
pub fn push(world: &mut World, state: impl GameState) {
    if !transitioning(world) {
        let frame = world.resource::<Time>().frame();
        world.resource_mut::<Shell>().change = Some(Change::Queued(frame));
        stack(world).request_push(state);
    }
}

/// Closes the screen on top: it fades and slides out, then pops; nothing
/// while another change runs.
pub fn pop(world: &mut World) {
    if transitioning(world) {
        return;
    }
    let tree = world.resource_mut::<UiTree>();
    let shown = |root: &&WidgetId| {
        tree.modal(**root) && tree.visibility(**root) == Some(Visibility::Visible)
    };
    if let Some(&top) = tree.roots().iter().rev().find(shown) {
        theme::slide_out(tree, top);
    }
    world.resource_mut::<Shell>().change = Some(Change::Closing(theme::POP_SECS));
}

/// Gives a screen's `root` to the state entering, drawn and frozen under
/// the screens above it, and slides it in.
pub fn enter(world: &mut World, root: WidgetId) {
    theme::slide_in(world.resource_mut::<UiTree>(), root);
    stack(world).own_root(root, RootCover::Show);
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

/// Quits to the title after one question, autosaving a live run first.
pub fn quit_to_title(world: &mut World) {
    quit(world, "Quit to the title?", |world| {
        goto(world, title::TitleState::default());
    });
}

/// Quits the game after one question, autosaving a live run first; the
/// exit's last frame autosaves again (`request_exit`).
pub fn quit_game(world: &mut World) {
    quit(world, "Quit the game?", request_exit);
}

/// Autosaves a live run, then asks `question`, naming the progress lost
/// when that autosave failed, and runs `then` on Quit.
fn quit(world: &mut World, question: &str, then: impl FnOnce(&mut World) + 'static) {
    let lost = save::capture(world).is_some() && !autosave(world);
    let lost = ["", " Progress since your last save will be lost."][usize::from(lost)];
    let ask = confirm::ConfirmState::new(&format!("{question}{lost}"), "Quit", "Stay");
    confirm::ask(world, ask.on_yes(then));
}

/// A menu screen a state owns: a modal root with a centred heading,
/// "Saving is off" without a user folder, and a column of buttons, sliding
/// in.
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
        let (root, card) = widgets::screen(tree, theme::MENU_WIDTH);
        widgets::heading(tree, card, heading, true);
        if !saving {
            widgets::note(tree, card, "Saving is off");
        }
        let buttons = captions
            .iter()
            .map(|caption| widgets::button(tree, card, caption));
        let buttons = buttons.collect();
        enter(world, root);
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
/// the menu sounds, the pause key and focus loss.
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
    menu_sounds(world);
    let open = idle(world) && top(world) == Some(game::GAMEPLAY);
    if open && pressed(world, "state_pause", true) {
        push(world, pause::PauseState::default());
    }
    focus(world, open);
}

/// Plays the manifest's sound `id` at its volume, looping as the manifest
/// says, faded in over `fade` seconds; `None` when it names no such sound.
pub fn play(world: &mut World, id: &str, fade: f32) -> Option<Voice> {
    let registry = world.resource::<SoundRegistry>();
    let handle = registry.get_by_id(id)?;
    let (volume, looping) = (registry.get_volume(handle), registry.get_looping(handle));
    let audio = world.resource_mut::<AudioCommands>();
    Some(audio.play_faded(handle, volume, looping, fade))
}

/// The menu sounds for this frame's routing: a move when a navigation
/// press moved focus, a select on an activation, a back on a cancel or a
/// Back button.
fn menu_sounds(world: &mut World) {
    let navigation = [
        "ui_up", "ui_down", "ui_left", "ui_right", "ui_next", "ui_prev",
    ];
    let navigated = navigation
        .iter()
        .any(|action| pressed(world, action, false));
    let tree = world.resource::<UiTree>();
    let mut heard = [false; 3];
    for event in events(world) {
        match event {
            UiEvent::FocusGained { .. } => heard[0] |= navigated,
            UiEvent::Activated { widget, .. } if tree.accessible_name(widget) == Some("Back") => {
                heard[2] = true;
            }
            UiEvent::Activated { .. } => heard[1] = true,
            UiEvent::Cancel { .. } => heard[2] = true,
            _ => {}
        }
    }
    for (sound, heard) in theme::MENU_SOUNDS.into_iter().zip(heard) {
        if heard {
            play(world, sound, 0.0);
        }
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

/// Whether a screen change waits or runs: an engine transition, a push or
/// pop the dispatcher has not applied, a closing screen or the curtain.
#[must_use]
pub fn transitioning(world: &World) -> bool {
    let frame = world.resource::<Time>().frame();
    let waiting = match &world.resource::<Shell>().change {
        Some(Change::Queued(at)) => *at == frame,
        change => change.is_some(),
    };
    waiting || world.resource::<StateStack>().is_transitioning()
}

/// Whether a widget is capturing the next press for a rebind.
#[must_use]
pub fn capturing(world: &World) -> bool {
    world.resource::<UiRouter>().capturing().is_some()
}

/// Whether a screen may act: no screen change or capture waits.
#[must_use]
pub fn idle(world: &World) -> bool {
    !transitioning(world) && !capturing(world)
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
