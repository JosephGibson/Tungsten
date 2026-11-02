//! The title: the game's name, large in the heading family, well above
//! Continue, New Game (through the difficulty picker, a card for each
//! difficulty with its rank and the game's note on it), Load, Settings and
//! Quit, smaller and spaced wider
//! than a menu's, with the version and "Saving is off" small at the bottom
//! left, and its song playing until a run starts. Continue loads the newest
//! readable slot, named with its age under it, asking first when an older
//! version wrote it, and is focused when there is one, disabled otherwise.
//! Quit asks first.

use tungsten::core::text::{TextAlign, TextWrap};
use tungsten::core::ui::{
    Align, Anchor, Edges, Justify, LayoutStyle, Sizing, StyleOverrides, UiKey, UiTree, WidgetId,
    widgets,
};
use tungsten::core::{AudioCommands, Voice, World};
use tungsten::{GameState, StateContext, StateId};

use super::confirm::{self, ConfirmState};
use super::options::GameOptions;
use super::saves::{Mode, SavesState, age, slot_name};
use super::settings::SettingsState;
use super::{
    AUTOSAVES, BACK, Difficulty, GameInfo, RunStart, RunState, Shell, SlotState, activated,
    cancelled, enter, idle, load, newest, play, push, quit_game, read_slots, saving_on, start_run,
    theme, toast,
};
use crate::game::difficulty_note;
use crate::save::SaveGame;

pub const TITLE: StateId = "title";
const CONTINUE: UiKey = UiKey::new("continue");
const NEW_GAME: UiKey = UiKey::new("new_game");
const LOAD: UiKey = UiKey::new("load");
const SETTINGS: UiKey = UiKey::new("settings");
const QUIT: UiKey = UiKey::new("quit");
/// The difficulty cards' keys, in [`Difficulty::ALL`]'s order.
const PICKS: [&str; 3] = ["easy", "normal", "hard"];

/// The title's widgets: the root, the main page and the picker, the main
/// page's Continue, New Game, Load, Settings and Quit, and the picker's
/// Easy, Normal, Hard and Back.
type Ui = (WidgetId, [WidgetId; 2], [WidgetId; 5], [WidgetId; 4]);

/// Keeps a label sized to its text on one line: such a box can commit an
/// ulp short of its text and wrap its last word out of sight (known issues).
fn one_line(tree: &mut UiTree, label: WidgetId) {
    widgets::restyle(tree, label, |text| text.wrap = Some(TextWrap::None));
}

/// The picker's card for `Difficulty::ALL[at]` under `row`: a button holding
/// its name in the heading family, its rank as filled squares of three, and
/// the game's note on it a fact a line (the note split at " · "), each
/// label as wide as the card, centred.
fn difficulty_card(tree: &mut UiTree, row: WidgetId, at: usize) -> WidgetId {
    let difficulty = Difficulty::ALL[at];
    let card = widgets::button(tree, row, difficulty.label());
    tree.set_key(card, PICKS[at]);
    let spacing = tree.theme().tokens.spacing;
    let mut style = tree.layout_style(card).cloned().unwrap_or_default();
    (style.width, style.gap) = (Sizing::Fill, spacing.sm);
    style.padding = Edges::all(spacing.md);
    tree.set_layout_style(card, style);
    let name = tree.children(card)[0];
    tree.set_text_style(name, Some("heading"));
    let pips = widgets::row(tree, card);
    let mut style = tree.layout_style(pips).cloned().unwrap_or_default();
    style.justify_content = Justify::Center;
    tree.set_layout_style(pips, style);
    let (accent, border) = {
        let colors = &tree.theme().tokens.colors;
        (colors.accent, colors.border)
    };
    let side = Sizing::Px(widgets::size(tree, "pip"));
    for each in 0..Difficulty::ALL.len() {
        let pip = tree.panel(pips, LayoutStyle::default().with_size(side, side));
        let filled = usize::from(each <= at);
        let style = StyleOverrides {
            background: Some([[0; 4], accent][filled]),
            border: Some([border, accent][filled]),
            border_width: Some(1.0),
            ..StyleOverrides::default()
        };
        tree.set_overrides(pip, style);
    }
    let facts = widgets::column(tree, card);
    let mut style = tree.layout_style(facts).cloned().unwrap_or_default();
    style.gap = 0.0;
    tree.set_layout_style(facts, style);
    for fact in difficulty_note(difficulty).split(" · ") {
        let note = widgets::note(tree, facts, fact);
        widgets::restyle(tree, note, |text| text.align = Some(TextAlign::Center));
    }
    card
}

/// The title screen.
#[derive(Default)]
pub struct TitleState {
    ui: Option<Ui>,
    newest: Option<(&'static str, SaveGame)>,
    autosave_ready: bool,
    picking: bool,
    opened: Option<WidgetId>,
    music: Option<Voice>,
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

    /// Starts a new run at `difficulty`, asking first while its autosaves
    /// will replace readable ones.
    fn new_game(&mut self, world: &mut World, difficulty: Difficulty) {
        let start = RunStart {
            difficulty,
            save: None,
        };
        if !self.autosave_ready {
            return start_run(world, start, None);
        }
        let question = "Start a new game? Its autosaves will replace the oldest ones.";
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
        let ready = |(slot, state): &(&str, SlotState)| {
            AUTOSAVES.contains(slot) && matches!(state, SlotState::Ready(..))
        };
        self.autosave_ready = world.resource::<GameOptions>().autosave && slots.iter().any(ready);
        let info = world.resource::<GameInfo>().clone();
        let saving = saving_on(world);
        let tree = world.resource_mut::<UiTree>();
        let size = |name| widgets::size(tree, name);
        let [gap, space, menu, wide] =
            ["title_gap", "title_space", "title_menu_width", "wide_width"].map(size);
        let [pad_x, pad_y] = ["button_padding_x", "title_button_padding_y"].map(size);
        // A clear card the viewport's width, so the name keeps one line.
        let width = size("menu_width");
        let (root, card) = widgets::screen(tree, width);
        widgets::fill(tree, card, [0; 4]);
        let mut style = tree.layout_style(card).cloned().unwrap_or_default();
        (style.max_width, style.align_items) = (None, Align::Center);
        style.gap = gap;
        tree.set_layout_style(card, style);
        // The card's width, wider than the name, which shrinks to one line.
        let title = widgets::display(tree, card, &info.title, "title");
        widgets::set_width(tree, title, Sizing::Percent(100.0));
        // The pages, centred under the name, the column as wide as the one
        // shown: the buttons' narrow, the picker's three cards a wide
        // screen's width.
        let column = widgets::column(tree, card);
        let mut style = tree.layout_style(column).cloned().unwrap_or_default();
        style.margin.top = space;
        style.align_items = Align::Center;
        tree.set_layout_style(column, style);
        let pages = [(); 2].map(|()| widgets::column(tree, column));
        for (page, width) in pages.into_iter().zip([menu, wide]) {
            let mut style = tree.layout_style(page).cloned().unwrap_or_default();
            (style.gap, style.width) = (gap, Sizing::Px(width));
            tree.set_layout_style(page, style);
        }
        let items = [
            ("continue", "Continue"),
            ("new_game", "New Game"),
            ("load", "Load"),
            ("settings", "Settings"),
            ("quit", "Quit"),
        ];
        let buttons = items.map(|(key, caption)| {
            let button = widgets::button(tree, pages[0], caption);
            tree.set_key(button, key);
            tree.edit_layout(button, |style| {
                style.padding = Edges::symmetric(pad_x, pad_y)
            });
            button
        });
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
        let cards = widgets::row(tree, pages[1]);
        let mut style = tree.layout_style(cards).cloned().unwrap_or_default();
        style.align_items = Align::Stretch;
        tree.set_layout_style(cards, style);
        let [easy, normal, hard] = [0, 1, 2].map(|at| difficulty_card(tree, cards, at));
        let back = widgets::button(tree, pages[1], "Back");
        tree.set_key(back, "back");
        let mut style = tree.layout_style(back).cloned().unwrap_or_default();
        (style.width, style.align_self) = (Sizing::Px(menu), Some(Align::Center));
        tree.set_layout_style(back, style);
        let picks = [easy, normal, hard, back];
        tree.set_enabled(buttons[0], self.newest.is_some());
        tree.set_enabled(buttons[2], saving);
        let corner = [tree.theme().tokens.spacing.lg; 2].into();
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
        self.music = play(world, theme::TITLE_MUSIC, theme::TITLE_FADE);
        let shell = world.resource_mut::<Shell>();
        if warnings > 0 && !std::mem::replace(&mut shell.warned, true) {
            let files = if warnings == 1 { "file" } else { "files" };
            toast::toast(world, format!("{warnings} save {files} could not be read"));
        }
    }

    fn on_exit(&mut self, ctx: &mut StateContext) {
        if let Some(music) = self.music.take() {
            let audio = ctx.world.resource_mut::<AudioCommands>();
            audio.stop_voice(music, theme::TITLE_FADE);
        }
    }

    fn on_resume(&mut self, ctx: &mut StateContext) {
        if let Some(button) = self.opened {
            ctx.world.resource_mut::<UiTree>().focus(button);
        }
    }

    fn update(&mut self, world: &mut World) {
        let Some((root, ..)) = self.ui.filter(|_| idle(world)) else {
            return;
        };
        let chosen = activated(world, root);
        let key = chosen.map(|(button, key)| {
            self.opened = Some(button);
            key
        });
        let pick = key.and_then(|key| PICKS.iter().position(|pick| UiKey::new(pick) == key));
        match (key, pick) {
            (_, Some(at)) => self.new_game(world, Difficulty::ALL[at]),
            (Some(BACK), _) => self.show_picker(world.resource_mut::<UiTree>(), false),
            (None, _) if self.picking && cancelled(world, root) => {
                self.show_picker(world.resource_mut::<UiTree>(), false);
            }
            (Some(CONTINUE), _) => {
                if let Some((slot, save)) = self.newest.clone() {
                    load(world, slot, save);
                }
            }
            (Some(NEW_GAME), _) => self.show_picker(world.resource_mut::<UiTree>(), true),
            (Some(LOAD), _) => push(world, SavesState::new(Mode::Load)),
            (Some(SETTINGS), _) => push(world, SettingsState::new(0)),
            (Some(QUIT), _) => quit_game(world),
            _ => {}
        }
    }
}
