//! The title: the game's name and version, Continue, New Game (through the
//! difficulty picker), Load, Settings and Quit. Continue loads the newest
//! readable slot and is focused when there is one, disabled otherwise.

use tungsten::core::World;
use tungsten::core::ui::{UiTree, WidgetId};
use tungsten::{GameState, RootCover, StateContext, StateId, request_exit};

use super::confirm::{self, ConfirmState};
use super::saves::{Mode, SavesState};
use super::settings::SettingsState;
use super::{
    Difficulty, GameInfo, RunStart, RunState, Shell, SlotState, cancelled, clicked, idle, newest,
    push, read_slots, saving_on, stack, start_run, theme, toast, widgets,
};
use crate::save::SaveGame;

pub const TITLE: StateId = "title";

/// The title's widgets: the root, the main page and the picker, the main
/// page's Continue, New Game, Load, Settings and Quit, and the picker's
/// Easy, Normal, Hard and Back.
type Ui = (WidgetId, [WidgetId; 2], [WidgetId; 5], [WidgetId; 4]);

/// The title screen.
#[derive(Default)]
pub struct TitleState {
    ui: Option<Ui>,
    newest: Option<(&'static str, SaveGame)>,
    autosave_ready: bool,
    picking: bool,
    opened: Option<WidgetId>,
}

impl TitleState {
    /// Shows the picker, or the main page, focused on Normal or New Game.
    fn show_picker(&mut self, tree: &mut UiTree, picking: bool) {
        let Some((_, [main, picker], buttons, picks)) = self.ui else {
            return;
        };
        self.picking = picking;
        widgets::show(tree, main, !picking);
        widgets::show(tree, picker, picking);
        tree.focus(if picking { picks[1] } else { buttons[1] });
    }

    /// Starts a new run at `difficulty`, asking first over a readable
    /// autosave, which the run's first frame replaces.
    fn new_game(&mut self, world: &mut World, difficulty: Difficulty) {
        let start = RunStart {
            difficulty,
            save: None,
        };
        if !self.autosave_ready {
            return start_run(world, start, None);
        }
        let question = "Start a new game? The autosave will be replaced.";
        let ask = ConfirmState::new(question, "Start", "Cancel");
        confirm::ask(
            world,
            ask.on_yes(move |world| start_run(world, start, None)),
        );
    }
}

impl GameState for TitleState {
    fn id(&self) -> StateId {
        TITLE
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        let world = &mut *ctx.world;
        world.remove_resource::<RunState>();
        theme::apply(world);
        let (slots, warnings) = read_slots(world);
        self.newest = newest(&slots);
        self.autosave_ready = matches!(slots[0].1, SlotState::Ready(..));
        let info = world.resource::<GameInfo>().clone();
        let saving = saving_on(world);
        let tree = world.resource_mut::<UiTree>();
        let root = widgets::screen(tree, theme::MENU_WIDTH);
        let size = [theme::TITLE_SIZE, theme::TITLE_LINE];
        widgets::sized_label(tree, root, &info.title, size);
        widgets::note(tree, root, &format!("Version {}", info.version));
        let pages = [(); 2].map(|()| widgets::column(tree, root));
        let captions = ["Continue", "New Game", "Load", "Settings", "Quit"];
        let buttons = captions.map(|caption| widgets::button(tree, pages[0], caption));
        widgets::heading(tree, pages[1], "Difficulty");
        let picks = ["Easy", "Normal", "Hard", "Back"]
            .map(|caption| widgets::button(tree, pages[1], caption));
        tree.set_enabled(buttons[0], self.newest.is_some());
        tree.set_enabled(buttons[2], saving);
        if !saving {
            widgets::note(tree, root, "Saving is off");
        }
        self.ui = Some((root, pages, buttons, picks));
        self.show_picker(tree, false);
        if self.newest.is_some() {
            tree.focus(buttons[0]);
        }
        theme::slide_in(tree, root);
        stack(world).own_root(root, RootCover::Hide);
        let shell = world.resource_mut::<Shell>();
        if warnings > 0 && !std::mem::replace(&mut shell.warned, true) {
            let files = if warnings == 1 { "file" } else { "files" };
            toast::toast(world, format!("{warnings} save {files} could not be read"));
        }
    }

    fn on_exit(&mut self, _ctx: &mut StateContext) {}

    fn on_resume(&mut self, ctx: &mut StateContext) {
        if let Some(button) = self.opened {
            ctx.world.resource_mut::<UiTree>().focus(button);
        }
    }

    fn update(&mut self, world: &mut World) {
        let Some((root, _, buttons, picks)) = self.ui.filter(|_| idle(world)) else {
            return;
        };
        let list: &[WidgetId] = if self.picking { &picks } else { &buttons };
        let pick = clicked(world, list);
        if let Some(at) = pick {
            self.opened = Some(list[at]);
        }
        match (self.picking, pick) {
            (true, Some(at @ 0..3)) => self.new_game(world, Difficulty::ALL[at]),
            (true, Some(_)) => self.show_picker(world.resource_mut::<UiTree>(), false),
            (true, None) if cancelled(world, root) => {
                self.show_picker(world.resource_mut::<UiTree>(), false);
            }
            (false, Some(0)) => {
                if let Some((slot, save)) = self.newest.clone() {
                    start_run(world, RunStart::from(save), Some(slot));
                }
            }
            (false, Some(1)) => self.show_picker(world.resource_mut::<UiTree>(), true),
            (false, Some(2)) => push(world, SavesState::new(Mode::Load)),
            (false, Some(3)) => push(world, SettingsState::new(0)),
            (false, Some(4)) => request_exit(world),
            _ => {}
        }
    }
}
