//! The title: the game's name, large in the heading family, above Continue,
//! New Game (through the difficulty picker, each difficulty beside the
//! game's note on it), Load, Settings and Quit, spaced wider than a menu's,
//! with the version and "Saving is off" small at the bottom left. Continue loads the newest readable slot,
//! named with its age under it, and is focused when there is one, disabled
//! otherwise. Quit asks first.

use tungsten::core::World;
use tungsten::core::text::TextWrap;
use tungsten::core::ui::{Align, Anchor, Justify, LayoutStyle, Sizing, UiTree, WidgetId};
use tungsten::{GameState, StateContext, StateId};

use super::confirm::{self, ConfirmState};
use super::saves::{Mode, SavesState, age, slot_name};
use super::settings::SettingsState;
use super::{
    Difficulty, GameInfo, RunStart, RunState, Shell, SlotState, cancelled, clicked, enter, idle,
    newest, push, quit_game, read_slots, saving_on, start_run, theme, toast, widgets,
};
use crate::game::difficulty_note;
use crate::save::SaveGame;

pub const TITLE: StateId = "title";

/// The title's widgets: the root, the main page and the picker, the main
/// page's Continue, New Game, Load, Settings and Quit, and the picker's
/// Easy, Normal, Hard and Back.
type Ui = (WidgetId, [WidgetId; 2], [WidgetId; 5], [WidgetId; 4]);

/// Keeps a label sized to its text on one line: such a box can commit an
/// ulp short of its text and wrap its last word out of sight (known issues).
fn one_line(tree: &mut UiTree, label: WidgetId) {
    widgets::restyle(tree, label, |text| text.wrap = Some(TextWrap::None));
}

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
        // A clear card the viewport's width, so the name keeps one line.
        let (root, card) = widgets::screen(tree, theme::MENU_WIDTH);
        widgets::fill(tree, card, [0; 4]);
        let mut style = tree.layout_style(card).cloned().unwrap_or_default();
        (style.max_width, style.align_items) = (None, Align::Center);
        style.gap = theme::TITLE_GAP;
        tree.set_layout_style(card, style);
        // The card's width, wider than the name: a box sized to its text
        // can commit an ulp short and wrap its last word out of sight.
        let title = widgets::display(tree, card, &info.title, theme::TITLE);
        widgets::set_width(tree, title, Sizing::Percent(100.0));
        let column = widgets::column(tree, card);
        widgets::set_width(tree, column, Sizing::Px(theme::MENU_WIDTH));
        let pages = [(); 2].map(|()| widgets::column(tree, column));
        for page in pages {
            let mut style = tree.layout_style(page).cloned().unwrap_or_default();
            style.gap = theme::TITLE_GAP;
            tree.set_layout_style(page, style);
        }
        let captions = ["Continue", "New Game", "Load", "Settings", "Quit"];
        let buttons = captions.map(|caption| widgets::button(tree, pages[0], caption));
        if let Some((slot, _)) = &self.newest {
            let at = slots.iter().find_map(|(each, state)| match state {
                SlotState::Ready(_, at) if each == slot => Some(*at),
                _ => None,
            });
            let line = widgets::row(tree, buttons[0]);
            let mut style = tree.layout_style(line).cloned().unwrap_or_default();
            style.justify_content = Justify::Center;
            tree.set_layout_style(line, style);
            let name = widgets::small(tree, line, &format!("{} ·", slot_name(slot)));
            let ago = widgets::small(tree, line, &age(at.unwrap_or(0)));
            widgets::mono(tree, ago);
            for label in [name, ago] {
                one_line(tree, label);
            }
        }
        widgets::heading(tree, pages[1], "Difficulty", true);
        let picks = ["Easy", "Normal", "Hard", "Back"].map(|caption| {
            let Some(at) = Difficulty::ALL.iter().position(|d| d.label() == caption) else {
                return widgets::button(tree, pages[1], caption);
            };
            let row = widgets::row(tree, pages[1]);
            let pick = widgets::button(tree, row, caption);
            widgets::set_width(tree, pick, Sizing::Fill);
            let note = widgets::note(tree, row, &difficulty_note(Difficulty::ALL[at]));
            one_line(tree, note);
            pick
        });
        tree.set_enabled(buttons[0], self.newest.is_some());
        tree.set_enabled(buttons[2], saving);
        let corner = [theme::MARGIN; 2].into();
        let footer = tree.panel(
            root,
            LayoutStyle::column().anchored(Anchor::BottomStart, corner),
        );
        widgets::fill(tree, footer, [0; 4]);
        widgets::small(tree, footer, &format!("Version {}", info.version));
        if !saving {
            widgets::small(tree, footer, "Saving is off");
        }
        self.ui = Some((root, pages, buttons, picks));
        self.show_picker(tree, false);
        if self.newest.is_some() {
            tree.focus(buttons[0]);
        }
        enter(world, root);
        widgets::capped(world, title, theme::TITLE, theme::TITLE_CAP);
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
            (false, Some(4)) => quit_game(world),
            _ => {}
        }
    }
}
