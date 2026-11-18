//! The shell: the screens every game keeps, copied unchanged into a new
//! game. A title, pause, game over, save and load, tabbed settings,
//! keybindings and toasts, with `tungsten::screens`' confirm, around a run
//! the game's gameplay state plays. It calls the game through seven names in
//! `game.rs` and `save.rs` (the seam) and nothing else outside this folder.
//!
//! A run starts with [`start_run`] and ends at the title. Saves go to
//! thirteen slots of `SaveSlots`: three autosaves, written in turn while the
//! Autosave option is on, when the game pauses, when it quits to the title
//! or exits, and once a run's first frame or the game's [`request_autosave`]
//! has a save to capture, and ten the player saves to. A slot that fails to
//! load (a newer build's, or a damaged file) is never loaded or overwritten
//! for the rest of the run of the program. One screen change runs at a time,
//! and screens act only when none runs (`tungsten::screens`): a push acts at
//! once, a pop after its screen fades out, and a change of run, through
//! [`goto`] or [`start_run`], behind a fade.

pub mod boot;
pub mod game_over;
pub mod keybindings;
pub mod options;
pub mod pause;
pub mod saves;
pub mod settings;
pub mod theme;
pub mod title;
pub mod toast;

use serde::{Deserialize, Serialize};
use tungsten::core::post::PostTail;
use tungsten::core::save::SaveError;
use tungsten::core::ui::{
    UiBindings, UiEvent, UiKey, UiRouter, UiTree, WidgetId, screen_build, widgets,
};
use tungsten::core::{
    ActionMap, EventQueue, InputState, RoutedInput, SaveSlots, Schedule, Stage, Time, Voice, World,
    system,
};
use tungsten::screens::{self, Confirm, ModalScreen, ScreensPlugin};
use tungsten::transition::TransitionPhase;
use tungsten::{
    DebugHud, Exiting, GameState, Plugin, StateId, StateStack, plugins::STATE_DISPATCHER,
    request_exit,
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

/// The `start_run` script action's arguments (S-80).
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScriptStart {
    #[serde(default)]
    difficulty: Difficulty,
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
    /// Written by an older version under a schema this build cannot read,
    /// with when.
    Older(u64),
    /// Written by a newer version of the game.
    Newer,
    Damaged,
}

/// The shell's own state: a `World` resource.
#[derive(Default)]
pub(crate) struct Shell {
    unfocused: bool,
    /// The window lost focus with "Pause when unfocused" on: pause opens
    /// once gameplay is on top with no screen change running.
    pause_due: bool,
    exit_saved: bool,
    /// The Settings screen is on the stack, so the exit saves its changes.
    pub(crate) settings_open: bool,
    pub(crate) warned: bool,
    /// Game over's entrance, a screen change: its real seconds so far and
    /// its root, whose buttons show once it ends.
    entrance: Option<(f32, WidgetId)>,
    /// Slots that failed to load: never loaded or written this run.
    pub(crate) read_only: Vec<&'static str>,
    /// The slots this program wrote, the first written first: of two saves
    /// of one time, the first written is the older.
    written: Vec<&'static str>,
    /// Real seconds since game over came on, while it is on the stack: the
    /// clock of its post passes.
    died: Option<f32>,
}

/// The shell as a plugin: `tungsten::screens`, the title first, the run's
/// systems, the options and the engine's HUD hidden until F4 shows it.
/// `tungsten.json`'s
/// `ui` names the theme, the UI's reference height, floor and default scale
/// (`D-184`) and the HUD's corner.
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
        ScreensPlugin.build(schedule, world);
        world.insert_resource(toast::Toasts::default());
        options::load(world);
        // The screen files' names (`assets/ui/screens/`).
        let saving_off = |world: &World| !saving_on(world);
        let bindings = UiBindings::of(world);
        bindings
            .enabled("saving_on", saving_on)
            .visible("saving_off", saving_off);
        settings::bind(bindings);
        title::bind(bindings);
        if let Some(hud) = world.get_resource_mut::<DebugHud>() {
            hud.enabled = false;
        }
        world.add_script_action_typed(
            "start_run",
            "Starts a fresh run: `{\"difficulty\"?: \"easy\" | \"normal\" | \"hard\"}`.",
            |world, start: ScriptStart| {
                let difficulty = start.difficulty;
                let save = None;
                start_run(world, RunStart { difficulty, save }, None);
                Ok(())
            },
        );
        schedule.add(Stage::Startup, system("shell_start", start));
        let save = system("shell_save", save_due).before(STATE_DISPATCHER);
        schedule.add(Stage::PreUpdate, save);
        schedule.add(Stage::Update, system("shell_frame", frame));
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

/// Starts a run behind the fade: the game's gameplay state replaces the
/// whole stack and its run state the old one at the swap, after the frame's
/// `shell_save` has saved the old run whole. `from` names the slot the save
/// came from, which Retry then loads. Nothing while another change runs.
pub fn start_run(world: &mut World, start: RunStart, from: Option<&'static str>) {
    if transitioning(world) {
        return;
    }
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
    screens::reset(world, game::gameplay(start), move |world| {
        world.insert_resource(run);
    });
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

/// Replaces the whole stack with `state` behind the fade
/// (`tungsten::screens::reset`): a screen's `update` runs off the stack, so
/// it cannot count the states itself. Nothing while another change runs.
pub fn goto(world: &mut World, state: impl GameState) {
    if !transitioning(world) {
        screens::reset(world, state, |_| {});
    }
}

/// Whether a change of run was asked and has not swapped yet: its fade
/// waits or still covers the old run.
#[must_use]
pub fn flow_pending(world: &World) -> bool {
    let stack = world.resource::<StateStack>();
    let phase = stack.transition_state().map(|state| state.phase);
    stack.is_transitioning() && phase != Some(TransitionPhase::In)
}

/// Pushes `state` over the screen on top, which stays drawn under its
/// scrim; nothing while another change runs.
pub fn push(world: &mut World, state: impl GameState) {
    if !transitioning(world) {
        screens::push(world, state);
    }
}

/// Closes the screen on top: it fades and slides out, then its own
/// `ModalScreen::update` pops it; nothing while another change runs.
pub fn pop(world: &mut World) {
    if !transitioning(world) {
        screens::pop(world);
    }
}

/// Gives a screen's `root` to the state entering, drawn and frozen under
/// the screens above it, and slides it in.
pub fn enter(world: &mut World, root: WidgetId) {
    ModalScreen::adopt(world, root, root);
}

/// Opens the game-over screen over gameplay: the game's gameplay state
/// calls it from its `update`, which runs off the stack, when its player
/// dies.
pub fn game_over(world: &mut World) {
    if !transitioning(world) && matches!(top(world), None | Some(game::GAMEPLAY)) {
        push(world, game_over::GameOverState::default());
    }
}

/// Whether the game has a user folder to save in.
#[must_use]
pub fn saving_on(world: &World) -> bool {
    world.resource::<SaveSlots>().dir().is_some()
}

/// Reads every slot of [`SLOTS`], in order, with [`OLD_AUTOSAVE`] after the
/// autosaves while its file stays; with how many files in the folder could
/// not be read.
pub fn read_slots(world: &mut World) -> (Vec<(&'static str, SlotState)>, usize) {
    let Some(dir) = world.resource::<SaveSlots>().dir() else {
        return (
            SLOTS.iter().map(|slot| (*slot, SlotState::Off)).collect(),
            0,
        );
    };
    let old = dir.join(format!("{OLD_AUTOSAVE}.json")).exists();
    let names = AUTOSAVES.into_iter().chain(old.then_some(OLD_AUTOSAVE));
    read_named(world, names.chain(SLOTS[AUTOSAVES.len()..].iter().copied()))
}

/// Reads `names`' slots, marking read-only the ones that fail to load but
/// an older schema's autosave, which the next autosave may take.
fn read_named(
    world: &mut World,
    names: impl Iterator<Item = &'static str>,
) -> (Vec<(&'static str, SlotState)>, usize) {
    let slots = world.resource::<SaveSlots>();
    let list = slots.list().unwrap_or_default();
    let time = |slot| list.slots.iter().find(|info| info.name == slot);
    let time = |slot| time(slot).map_or(0, |info| info.saved_at_ms);
    let read = |slot: &'static str| {
        let state = match slots.load::<SaveGame>(slot) {
            Ok(save) => SlotState::Ready(Box::new(save), time(slot)),
            Err(SaveError::NotFound { .. }) => SlotState::Empty,
            Err(SaveError::NoFolder) => SlotState::Off,
            Err(SaveError::NewerSchema { .. } | SaveError::UnknownEnvelope { .. }) => {
                SlotState::Newer
            }
            Err(error) => {
                log::warn!("Save slot '{slot}' not loaded: {error}");
                match error {
                    SaveError::Migration { .. } => SlotState::Older(time(slot)),
                    _ => SlotState::Damaged,
                }
            }
        };
        (slot, state)
    };
    let scan: Vec<_> = names.map(read).collect();
    let shell = world.resource_mut::<Shell>();
    for (slot, state) in &scan {
        let failed = match state {
            SlotState::Newer | SlotState::Damaged => true,
            SlotState::Older(_) => !AUTOSAVES.contains(slot),
            _ => false,
        };
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
    let ask = Confirm::new(&question, "Load", "Cancel");
    screens::ask(
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
/// first autosave with no save, else the oldest an older schema left, else
/// the oldest, else a damaged one; never a newer version's; `false` when
/// nothing was written.
pub fn autosave(world: &mut World) -> bool {
    world.resource::<options::GameOptions>().autosave
        && save::capture(world).is_some_and(|save| write_autosave(world, &save))
}

/// Writes `save` to the autosave slot [`autosave`] names, skipping
/// read-only ones but a damaged one, the last resort, which stops being
/// read-only; of two saves of one time, the one this program wrote first
/// is the older, then the lower number.
fn write_autosave(world: &mut World, save: &SaveGame) -> bool {
    let last = world.resource::<RunState>().last_autosave;
    let slot = last.map(|slot| (slot, false)).or_else(|| {
        let (slots, _) = read_named(world, AUTOSAVES.into_iter());
        let shell = world.resource::<Shell>();
        let order = |slot| shell.written.iter().position(|each| *each == slot);
        let free = |slot| !shell.read_only.contains(&slot);
        let keys = slots
            .into_iter()
            .enumerate()
            .filter_map(|(at, (slot, state))| {
                let key = match state {
                    SlotState::Empty if free(slot) => (0, 0, None, at),
                    SlotState::Older(time) if free(slot) => (1, time, None, at),
                    SlotState::Ready(_, time) if free(slot) => (2, time, order(slot), at),
                    SlotState::Damaged => (3, 0, None, at),
                    _ => return None,
                };
                Some((key, slot))
            });
        let (key, slot) = keys.min_by_key(|(key, _)| *key)?;
        Some((slot, key.0 == 3))
    });
    let Some((slot, damaged)) = slot else {
        log::warn!("No autosave slot can be written");
        return false;
    };
    if damaged {
        world
            .resource_mut::<Shell>()
            .read_only
            .retain(|each| *each != slot);
    }
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
    let ask = Confirm::new(&format!("{question}{lost}"), "Quit", "Stay");
    screens::ask(world, ask.on_yes(then));
}

/// A menu screen a state owns, built from its screen file in
/// `assets/ui/screens/` (its keyed buttons, the bindings the shell
/// registers), its handlers reading keys.
pub struct Menu {
    pub root: WidgetId,
    /// The file's revision the state last saw: a hot reload rebuilds it.
    revision: u32,
    /// The button that opened the screen above, which takes focus back.
    opened: Option<WidgetId>,
    /// The modal screen [`Menu::new`] adopted it as (`D-236`), which pops it
    /// on a cancel or Back and once its slide out ends.
    screen: Option<ModalScreen>,
}

impl Menu {
    /// The screen file `id` built and adopted as a modal screen, sliding
    /// in, its first button focused.
    pub fn new(world: &mut World, id: &str) -> Self {
        let mut menu = Self::build(world, id);
        let card = screen_build::node(world, menu.root, "card").unwrap_or(menu.root);
        menu.screen = Some(ModalScreen::adopt(world, menu.root, card));
        menu.focus(world);
        menu
    }

    /// The screen file `id` built; an empty screen when it is not loaded.
    pub fn build(world: &mut World, id: &str) -> Self {
        let built = screen_build::build(world, id).map(|built| built.root);
        let tree = world.resource_mut::<UiTree>();
        let root = built.unwrap_or_else(|| widgets::screen(tree, 0.0).0);
        let (revision, opened, screen) = (0, None, None);
        Self {
            root,
            revision,
            opened,
            screen,
        }
    }

    /// Whether a reload rebuilt the screen since the last call.
    pub fn revised(&mut self, world: &World) -> bool {
        let revision = screen_build::revision(world, self.root).unwrap_or(self.revision);
        std::mem::replace(&mut self.revision, revision) != revision
    }

    /// Focuses the button that opened the screen above, else the first that
    /// can take focus, depth first.
    pub fn focus(&self, world: &mut World) {
        if let Some(screen) = &self.screen {
            return screen.resume(world);
        }
        let tree = world.resource_mut::<UiTree>();
        let (mut stack, mut nodes) = (vec![self.root], Vec::new());
        while let Some(id) = stack.pop() {
            nodes.push(id);
            stack.extend(tree.children(id).iter().rev());
        }
        widgets::focus_first(tree, self.opened.into_iter().chain(nodes));
    }

    /// The key of the button activated on the menu at this routing, which
    /// then takes focus back when the screen above closes; for an adopted
    /// menu, its `ModalScreen::update`, which closes it on a cancel and pops
    /// it once its slide out ends, `None` while it is not idle.
    pub fn chosen(&mut self, world: &mut World) -> Option<UiKey> {
        if let Some(screen) = self.screen.as_mut() {
            return screen.update(world);
        }
        let (button, key) = activated(world, self.root)?;
        self.opened = Some(button);
        Some(key)
    }
}

/// `shell_save`, before the dispatcher, so a capture reads the world as the
/// last frame's command flush left it: the run's due autosave once
/// `capture` returns a save, even if the write then fails, or its exit
/// save. A change of run's swap follows it in the dispatcher, so the save
/// holds the old run whole.
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
}

/// `shell_frame`: game over's passes, the run's playtime, the exit's save
/// of the Settings screen's changes while it is open, after a confirm's
/// safe answer, the pause key, focus loss, the end of game over's entrance
/// and the toasts.
fn frame(world: &mut World) {
    let real = world.resource::<Time>().real_delta();
    let died = world.resource_mut::<Shell>().died.as_mut().map(|secs| {
        *secs += real;
        *secs
    });
    // Game over's passes, which draw after the game's stack while it shows.
    if let (Some(secs), Some(tail)) = (died, world.get_resource_mut::<PostTail>()) {
        tail.0.clear();
        tail.0.extend(game_over::passes(secs));
    }
    let delta = f64::from(world.resource::<Time>().delta());
    if let Some(run) = world.get_resource_mut::<RunState>() {
        run.playtime_secs += delta;
    }
    let exit = world.has_resource::<Exiting>()
        && !std::mem::replace(&mut world.resource_mut::<Shell>().exit_saved, true);
    if exit && world.resource::<Shell>().settings_open {
        settings::save(world);
    }
    let open = idle(world) && top(world) == Some(game::GAMEPLAY);
    if open && pressed(world, "state_pause", true) {
        push(world, pause::PauseState::default());
    }
    focus(world, open);
    // Game over's entrance: its buttons show once its seconds run out.
    let shell = world.resource_mut::<Shell>();
    match shell.entrance.take() {
        Some((secs, root)) if secs + real < theme::DIED_SECS => {
            shell.entrance = Some((secs + real, root));
        }
        Some((_, root)) => game_over::reveal(world, root),
        None => {}
    }
    toast::tick(world);
}

/// Plays the manifest's sound `id` at its volume, looping as the manifest
/// says, faded in over `fade` seconds; `None` when it names no such sound.
pub fn play(world: &mut World, id: &str, fade: f32) -> Option<Voice> {
    tungsten::core::audio::play(world, id, fade)
}

/// Focus loss: pauses gameplay when the options say so, once `open`
/// (gameplay on top, no screen change), so a loss during a change pauses
/// after it; focus back drops a pause still due. The mute while unfocused
/// is the engine's (`Settings::mute_unfocused`).
fn focus(world: &mut World, open: bool) {
    let unfocused = !world.resource::<InputState>().focused();
    let pause = world.resource::<options::GameOptions>().pause_unfocused;
    let shell = world.resource_mut::<Shell>();
    if std::mem::replace(&mut shell.unfocused, unfocused) != unfocused {
        shell.pause_due = unfocused && pause;
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

/// Whether a screen change waits or runs: a push, pop or change of run
/// the stack has not finished (`tungsten::screens`), a closing screen, or
/// game over's entrance.
#[must_use]
pub fn transitioning(world: &World) -> bool {
    screens::transitioning(world) || world.resource::<Shell>().entrance.is_some()
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

/// The key every screen's Back button takes, the engine's back role: it
/// backs out of its screen and plays the theme's back sound.
pub const BACK: UiKey = UiKey::new("back");

/// The keyed control on `root` activated at this routing, with its key.
#[must_use]
pub fn activated(world: &World, root: WidgetId) -> Option<(WidgetId, UiKey)> {
    let tree = world.resource::<UiTree>();
    let mut events = world.resource::<EventQueue<UiEvent>>().iter_current();
    events.find_map(|event| match *event {
        UiEvent::Activated {
            widget,
            key: Some(key),
            ..
        } if tree.root_of(widget) == Some(root) => Some((widget, key)),
        _ => None,
    })
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
