//! The shell: the screens every game keeps, copied unchanged into a new
//! game. A title, pause, game over, save and load, tabbed settings,
//! keybindings, a confirm dialog and toasts, around a run the game's
//! gameplay state plays. It calls the game through seven names in
//! `game.rs` and `save.rs` (the seam) and nothing else outside this folder.
//!
//! A run starts with [`start_run`] and ends at the title. Saves go to
//! thirteen slots of `SaveSlots`: three autosaves, written in turn while the
//! Autosave option is on, when the game pauses, when it quits to the title
//! or exits, and once a run's first frame or the game's [`request_autosave`]
//! has a save to capture, and ten the player saves to. A slot that fails to
//! load (a newer build's, or a damaged file) is never loaded or overwritten
//! for the rest of the run of the program. One screen change runs at a time, and screens act only when
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
use tungsten::core::post::{PostPass, PostStack};
use tungsten::core::save::SaveError;
use tungsten::core::ui::{UiEvent, UiRouter, UiTrack, UiTree, Visibility, WidgetId};
use tungsten::core::{
    ActionMap, AudioCommands, DisplayMode, DisplayState, EventQueue, InputState, RoutedInput,
    SaveSlots, ScaleFactor, Schedule, Settings, SoundRegistry, Stage, Time, Voice, World, system,
};
use tungsten::{
    DebugHud, Exiting, GameState, HudCorner, Plugin, RootCover, StateId, StateStack,
    plugins::STATE_DISPATCHER, request_exit,
};

use crate::game;
use crate::save::{self, SaveGame};

/// The autosaves' slots, which the shell writes in turn.
pub const AUTOSAVES: [&str; 3] = ["autosave_1", "autosave_2", "autosave_3"];
/// The one autosave of the shell before 0.94: its start renames the file to
/// an autosave slot with no file, and lists one that stays, never writing it.
pub const OLD_AUTOSAVE: &str = "autosave";
/// Every slot the shell writes, the autosaves first.
pub const SLOTS: [&str; 13] = [
    AUTOSAVES[0],
    AUTOSAVES[1],
    AUTOSAVES[2],
    "slot_1",
    "slot_2",
    "slot_3",
    "slot_4",
    "slot_5",
    "slot_6",
    "slot_7",
    "slot_8",
    "slot_9",
    "slot_10",
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
    /// An autosave is due, the run's first or a requested one: it waits
    /// until `capture` returns a save.
    autosave_due: bool,
    /// The autosave slot this run wrote last, until the next fixed step: an
    /// autosave before then rewrites it, so a pause and a quit take one.
    last_autosave: Option<&'static str>,
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
/// the curtain: the replace, and the incoming run's state, staged until
/// `shell_save` has run before the replace, so a save holds one run whole.
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
    /// Game over's entrance: its seconds so far and its buttons, which show
    /// once it ends.
    Entrance(f32, WidgetId),
}

/// The shell's own state: a `World` resource.
#[derive(Default)]
pub(crate) struct Shell {
    unfocused: bool,
    /// The window lost focus with "Mute when unfocused" on.
    focus_muted: bool,
    /// The window lost focus with "Pause when unfocused" on: pause opens
    /// once gameplay is on top with no screen change running.
    pause_due: bool,
    exit_saved: bool,
    /// The Settings screen is on the stack, so the exit saves its changes.
    pub(crate) settings_open: bool,
    pub(crate) warned: bool,
    change: Option<Change>,
    /// The run a change of run swapped in, live after the next `shell_save`.
    incoming: Option<RunState>,
    /// Slots that failed to load: never loaded or written this run.
    pub(crate) read_only: Vec<&'static str>,
    /// The slots this program wrote, the first written first: of two saves
    /// of one time, the first written is the older.
    written: Vec<&'static str>,
    post: Post,
    /// Real seconds since game over came on, while it is on the stack: the
    /// clock of its post passes.
    died: Option<f32>,
}

/// The shell's edits of `PostStack`, which [`post`] makes.
#[derive(Default)]
struct Post {
    /// The game's Bloom passes taken out while the option is off, each with
    /// its index in the game's stack.
    kept: Vec<(usize, PostPass)>,
    /// The passes the shell appended last frame.
    tail: Vec<PostPass>,
    /// The stack as the shell left it last frame.
    left: Vec<PostPass>,
}

/// The shell as a plugin: the title first, the run's systems, the options
/// store, the engine's HUD at the top right, hidden until F4 shows it, and
/// the UI sized from the window's height under a floor (`D-184`), at
/// [`theme::UI_SCALE`] while the player has set no UI scale.
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
        world.insert_resource(widgets::Capped::default());
        options::load(world);
        let unset = world.resource::<Settings>().ui_scale().is_none();
        if let Some(scale) = world.get_resource_mut::<ScaleFactor>() {
            scale.set_reference_height(Some(theme::REFERENCE_HEIGHT));
            scale.set_min_viewport(Some(theme::MIN_VIEWPORT.into()));
            if unset {
                scale.set_ui_scale(theme::UI_SCALE);
            }
        }
        if let Some(hud) = world.get_resource_mut::<DebugHud>() {
            hud.enabled = false;
            hud.corner = HudCorner::TopRight;
        }
        schedule.add(Stage::Startup, system("shell_start", start));
        let save = system("shell_save", save_due).before(STATE_DISPATCHER);
        schedule.add(Stage::PreUpdate, save);
        schedule.add(Stage::Update, system("shell_frame", frame));
        schedule.add(Stage::Update, system("shell_flow", flow));
        schedule.add(Stage::FixedUpdate, system("shell_checkpoint", checkpoint));
    }
}

/// The startup system: the options applied, then the title. The harness runs
/// `Startup` on its first step.
pub fn start(world: &mut World) {
    options::apply(world);
    adopt_old_autosave(world);
    stack(world).request_push(title::TitleState::default());
}

/// Renames the shell's old `autosave` file to the first autosave slot with
/// no file, which keeps its data and its time; a file that cannot move
/// stays, listed under its old name.
fn adopt_old_autosave(world: &World) {
    let Some(dir) = world.resource::<SaveSlots>().dir() else {
        return;
    };
    let file = |slot: &str| dir.join(format!("{slot}.json"));
    let old = file(OLD_AUTOSAVE);
    let free = AUTOSAVES.map(file).into_iter().find(|to| !to.exists());
    if let (true, Some(to)) = (old.is_file(), free)
        && let Err(error) = std::fs::rename(&old, &to)
    {
        log::warn!("The old autosave stays where it is: {error}");
    }
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
        autosave_due: true,
        last_autosave: None,
    };
    set_flow(world, game::gameplay(start), Some(run));
}

/// Asks for an autosave at the start of the next frame whose `capture`
/// returns a save; nothing with no live run or during a change of run, whose
/// first autosave covers it, and dropped while the Autosave option is off.
pub fn request_autosave(world: &mut World) {
    if !flow_pending(world)
        && let Some(run) = world.get_resource_mut::<RunState>()
    {
        run.autosave_due = true;
    }
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

/// Whether a [`goto`] waits for the curtain, or its run for the replace.
#[must_use]
pub fn flow_pending(world: &World) -> bool {
    let shell = world.resource::<Shell>();
    matches!(shell.change, Some(Change::Curtain(_, _, Some(_)))) || shell.incoming.is_some()
}

/// `shell_flow`: the closing screen's pop, the curtain's swap and the end of
/// game over's entrance once their time comes, then the toasts, which stay
/// above the curtain.
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
        Some(Change::Entrance(secs, card)) if secs + dt < theme::DIED_SECS => {
            shell.change = Some(Change::Entrance(secs + dt, card));
        }
        Some(Change::Entrance(_, card)) => game_over::reveal(world, card),
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
        // The game's stack back as it built it, before its run leaves.
        post(world, Vec::new(), true);
        let stack = stack(world);
        for _ in 1..stack.depth() {
            stack.request_pop();
        }
        replace(stack);
        world.resource_mut::<Shell>().incoming = run;
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

/// Opens the game-over screen over gameplay: the game's gameplay state
/// calls it from its `update`, which runs off the stack, when its player
/// dies.
pub fn game_over(world: &mut World) {
    if !transitioning(world) && matches!(top(world), None | Some(game::GAMEPLAY)) {
        push(world, game_over::GameOverState::default());
    }
}

/// Applies `Settings` through the engine, keeping the focus mute: while it
/// holds, the engine sees a copy at volume 0, so no other master volume is
/// queued. An unset UI scale applies as [`theme::UI_SCALE`].
pub fn apply_settings(world: &mut World) {
    let saved = world.resource::<Settings>().clone();
    let unset = saved.ui_scale().is_none();
    if world.resource::<Shell>().focus_muted {
        let mut quiet = saved.clone();
        quiet.set_master_volume(Some(0.0));
        world.insert_resource(quiet);
        tungsten::apply_settings(world);
        world.insert_resource(saved);
    } else {
        tungsten::apply_settings(world);
    }
    if let (true, Some(scale)) = (unset, world.get_resource_mut::<ScaleFactor>()) {
        scale.set_ui_scale(theme::UI_SCALE);
    }
}

/// Whether the game has a user folder to save in.
#[must_use]
pub fn saving_on(world: &World) -> bool {
    world.resource::<SaveSlots>().dir().is_some()
}

/// Reads every slot of [`SLOTS`], in order, with [`OLD_AUTOSAVE`] after the
/// autosaves while its file stays, marking the ones that fail to load
/// read-only; with how many files in the folder could not be read.
pub fn read_slots(world: &mut World) -> (Vec<(&'static str, SlotState)>, usize) {
    let slots = world.resource::<SaveSlots>();
    let Some(dir) = slots.dir() else {
        return (
            SLOTS.iter().map(|slot| (*slot, SlotState::Off)).collect(),
            0,
        );
    };
    let old = dir.join(format!("{OLD_AUTOSAVE}.json")).exists();
    let names = AUTOSAVES.into_iter().chain(old.then_some(OLD_AUTOSAVE));
    let names = names.chain(SLOTS[AUTOSAVES.len()..].iter().copied());
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
    let scan: Vec<_> = names.map(read).collect();
    let shell = world.resource_mut::<Shell>();
    for (slot, state) in &scan {
        let failed = matches!(state, SlotState::Newer | SlotState::Damaged);
        if failed && !shell.read_only.contains(slot) {
            shell.read_only.push(slot);
        }
    }
    (scan, list.warnings.len())
}

/// A version's leading dot-separated numbers, a pre-release or build suffix
/// dropped; `None` when they do not parse.
fn version_numbers(version: &str) -> Option<Vec<u64>> {
    let core = version.split(['-', '+']).next()?;
    core.split('.').map(|part| part.parse().ok()).collect()
}

/// Whether a save written by version `saved` is older than `current`: their
/// numbers compare in order, a suffix ignored; one that does not parse, such
/// as the migrated `"unknown"`, is older.
#[must_use]
pub fn older(saved: &str, current: &str) -> bool {
    match (version_numbers(saved), version_numbers(current)) {
        (Some(saved), Some(current)) => saved < current,
        (saved, _) => saved.is_none(),
    }
}

/// A save's version on its row: none for this version or a newer one.
#[must_use]
pub fn version_note(world: &World, saved: &str) -> Option<String> {
    let current = &world.resource::<GameInfo>().version;
    match version_numbers(saved) {
        _ if !older(saved, current) => None,
        Some(_) => Some(format!("Older version {saved}")),
        None => Some("Unknown version".to_owned()),
    }
}

/// Starts a run from `slot`'s `save`, asking first, in one question, when
/// an older version wrote it or a live run would lose its progress since
/// its last save; Retry loads with no question.
pub fn load(world: &mut World, slot: &'static str, save: SaveGame) {
    let checkpointed = run(world).is_some_and(|run| run.checkpointed);
    let lose = save::capture(world).is_some() && !checkpointed;
    let name = saves::slot_name(slot);
    let version = &save.meta.game_version;
    let older = version_note(world, version).is_some();
    let shown = [version.as_str(), "unknown"][usize::from(version.is_empty())];
    let mut question = match older {
        true => format!("{name} was saved by an older version, {shown}. Load it?"),
        false => format!("Load {name}?"),
    };
    let start = RunStart::from(save);
    if !older && !lose {
        return start_run(world, start, Some(slot));
    }
    if lose {
        question.push_str(" Progress since your last save will be lost.");
    }
    let ask = confirm::ConfirmState::new(&question, "Load", "Cancel");
    confirm::ask(
        world,
        ask.on_yes(move |world| start_run(world, start, Some(slot))),
    );
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
    save::capture(world).is_some_and(|save| write(world, slot, &save))
}

/// Writes `save` to `slot`, as [`save_to`] does once it has one.
fn write(world: &mut World, slot: &'static str, save: &SaveGame) -> bool {
    if world.resource::<Shell>().read_only.contains(&slot) {
        return false;
    }
    match world.resource::<SaveSlots>().save(slot, save) {
        Ok(()) => {
            let run = world.resource_mut::<RunState>();
            run.checkpointed = true;
            run.retry_from = Some(slot);
            let written = &mut world.resource_mut::<Shell>().written;
            written.retain(|each| *each != slot);
            written.push(slot);
            true
        }
        Err(SaveError::NoFolder) => false,
        Err(error) => {
            log::warn!("Save slot '{slot}' not saved: {error}");
            false
        }
    }
}

/// Saves the live run to an autosave while the Autosave option is on: to
/// the slot the run wrote last when no fixed step has run since, else the
/// first autosave with no save, else the oldest; `false` when nothing was
/// written.
pub fn autosave(world: &mut World) -> bool {
    world.resource::<options::GameOptions>().autosave
        && save::capture(world).is_some_and(|save| write_autosave(world, &save))
}

/// Writes `save` to the autosave slot [`autosave`] names, skipping
/// read-only ones; of two saves of one time, the one this program wrote
/// first is the older, then the lower number.
fn write_autosave(world: &mut World, save: &SaveGame) -> bool {
    let last = world.resource::<RunState>().last_autosave;
    let slot = last.or_else(|| {
        let (slots, _) = read_slots(world);
        let shell = world.resource::<Shell>();
        let order = |slot| shell.written.iter().position(|each| *each == slot);
        let free = |slot| AUTOSAVES.contains(&slot) && !shell.read_only.contains(&slot);
        let keys = slots
            .into_iter()
            .enumerate()
            .filter_map(|(at, (slot, state))| {
                let key = match state {
                    SlotState::Empty => (false, 0, None, at),
                    SlotState::Ready(_, time) => (true, time, order(slot), at),
                    _ => return None,
                };
                free(slot).then_some((key, slot))
            });
        keys.min_by_key(|(key, _)| *key).map(|(_, slot)| slot)
    });
    let Some(slot) = slot else {
        log::warn!("No autosave slot can be written");
        return false;
    };
    let written = write(world, slot, save);
    if written {
        world.resource_mut::<RunState>().last_autosave = Some(slot);
    }
    written
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
    pub heading: WidgetId,
    pub buttons: Vec<WidgetId>,
    /// The button that opened the screen above, which takes focus back.
    opened: Option<WidgetId>,
}

impl Menu {
    pub fn new(world: &mut World, heading: &str, captions: &[&str]) -> Self {
        let saving = saving_on(world);
        let tree = world.resource_mut::<UiTree>();
        let (root, card) = widgets::screen(tree, theme::MENU_WIDTH);
        let heading = widgets::heading(tree, card, heading, true);
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
            heading,
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

/// `shell_save`, before the dispatcher, so a capture reads the world as the
/// last frame's command flush left it: the run's due autosave once
/// `capture` returns a save, even if the write then fails, or its exit
/// save; then a staged run goes live for its replace.
fn save_due(world: &mut World) {
    let due = world
        .get_resource::<RunState>()
        .is_some_and(|run| run.autosave_due);
    let exit = world.has_resource::<Exiting>() && !world.resource::<Shell>().exit_saved;
    let on = world.resource::<options::GameOptions>().autosave;
    if due && !on {
        world.resource_mut::<RunState>().autosave_due = false;
    }
    if let Some(save) = (on && (due || exit))
        .then(|| save::capture(world))
        .flatten()
    {
        world.resource_mut::<RunState>().autosave_due = false;
        write_autosave(world, &save);
    }
    if let Some(run) = world.resource_mut::<Shell>().incoming.take() {
        world.insert_resource(run);
    }
}

/// The shell's one edit of `PostStack` a frame. A stack as the shell left it
/// loses the passes the shell appended; one the game rebuilt drops the
/// Bloom the shell took, and those passes only if it still ends with them.
/// While the Bloom option is off the game's Bloom passes are taken out,
/// each with its index, and put back at it when the option comes on;
/// then `tail` goes at the end. At a change of run (`swap`) the stack goes
/// back to the game's own, as it built it.
fn post(world: &mut World, tail: Vec<PostPass>, swap: bool) {
    let bloom = swap || world.resource::<options::GameOptions>().bloom;
    let mut post = std::mem::take(&mut world.resource_mut::<Shell>().post);
    let stack = &mut world.resource_mut::<PostStack>().0;
    if *stack != post.left {
        post.kept.clear();
    }
    if stack.ends_with(&post.tail) {
        stack.truncate(stack.len() - post.tail.len());
    }
    if bloom {
        for (at, pass) in std::mem::take(&mut post.kept) {
            stack.insert(at.min(stack.len()), pass);
        }
    } else {
        for (at, pass) in std::mem::take(stack).into_iter().enumerate() {
            match pass {
                PostPass::Bloom(_) => post.kept.push((at, pass)),
                pass => stack.push(pass),
            }
        }
    }
    stack.extend(tail.iter().copied());
    (post.tail, post.left) = (tail, stack.clone());
    world.resource_mut::<Shell>().post = post;
}

/// `shell_frame`: the F11 and F9 keys, the run's playtime, the exit's save
/// of the Settings screen's changes while it is open, after a confirm's
/// safe answer, the menu sounds, the pause key, focus loss and the post
/// stack.
fn frame(world: &mut World) {
    widgets::recap(world);
    display_keys(world);
    let real = world.resource::<Time>().real_delta();
    let died = world.resource_mut::<Shell>().died.as_mut().map(|secs| {
        *secs += real;
        *secs
    });
    post(world, died.map_or_else(Vec::new, game_over::passes), false);
    let delta = f64::from(world.resource::<Time>().delta());
    if let Some(run) = world.get_resource_mut::<RunState>() {
        run.playtime_secs += delta;
    }
    let exit = world.has_resource::<Exiting>()
        && !std::mem::replace(&mut world.resource_mut::<Shell>().exit_saved, true);
    if exit && world.resource::<Shell>().settings_open {
        settings::save(world);
    }
    menu_sounds(world);
    let open = idle(world) && top(world) == Some(game::GAMEPLAY);
    if open && pressed(world, "state_pause", true) {
        push(world, pause::PauseState::default());
    }
    focus(world, open);
}

/// F11 and F9, the engine's fullscreen and vsync keys, kept in `Settings`
/// and saved as the engine toggles them, so the Display rows show them and
/// the next apply, or start, keeps them.
fn display_keys(world: &mut World) {
    let keys = ["engine_toggle_fullscreen", "engine_toggle_vsync"];
    let [full, vsync] = keys.map(|key| pressed(world, key, false));
    if !full && !vsync {
        return;
    }
    // The engine toggles the live display; its request lands next frame.
    let live = *world.resource::<DisplayState>();
    let settings = world.resource_mut::<Settings>();
    let modes = [DisplayMode::Windowed, DisplayMode::BorderlessFullscreen];
    if full {
        settings.set_display_mode(Some(modes[usize::from(live.display_mode == modes[0])]));
    }
    if vsync {
        settings.set_vsync(Some(!live.vsync));
    }
    // Applied again, since an apply earlier this frame, a slider's, replaces
    // the engine's request; the Settings screen saves when it closes.
    apply_settings(world);
    if !world.resource::<Shell>().settings_open {
        settings::save(world);
    }
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

/// Focus loss: mutes when the options say so, and pauses gameplay once
/// `open` (gameplay on top, no screen change), so a loss during a change
/// pauses after it; focus back unmutes and drops a pause still due.
fn focus(world: &mut World, open: bool) {
    let unfocused = !world.resource::<InputState>().focused();
    let options = *world.resource::<options::GameOptions>();
    let shell = world.resource_mut::<Shell>();
    if std::mem::replace(&mut shell.unfocused, unfocused) != unfocused {
        shell.pause_due = unfocused && options.pause_unfocused;
        let muted = unfocused && options.mute_unfocused;
        if std::mem::replace(&mut shell.focus_muted, muted) != muted {
            apply_settings(world);
        }
    }
    if open && std::mem::take(&mut world.resource_mut::<Shell>().pause_due) {
        push(world, pause::PauseState::default());
    }
}

/// `shell_checkpoint`: a fixed step ran, so the run has moved past its last
/// save, and its next autosave takes a slot of its own.
fn checkpoint(world: &mut World) {
    if let Some(run) = world.get_resource_mut::<RunState>() {
        (run.checkpointed, run.last_autosave) = (false, None);
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
