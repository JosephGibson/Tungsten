//! The title: the game's name, large in the heading family, well above
//! Continue, New Game (through the difficulty picker, a card for each
//! difficulty with its rank and the game's note on it), Load, Settings and
//! Quit, smaller and spaced wider
//! than a menu's, with the version and "Saving is off" small at the bottom
//! left, and its song playing until a run starts. Continue loads the newest
//! readable slot, named with its age under it, asking first when an older
//! version wrote it, and is focused when there is one, disabled otherwise.
//! Quit asks first. The screen is `assets/ui/screens/title.json`, its
//! values the bindings [`bind`] registers over a [`TitleView`]; the cards
//! stay Rust, a mount, since their pips fill by rank.

use tungsten::core::World;
use tungsten::core::text::TextAlign;
use tungsten::core::ui::{
    Edges, Justify, LayoutStyle, Sizing, StyleOverrides, UiBindings, UiKey, UiTree, WidgetId,
    screen_build, widgets,
};
use tungsten::{GameState, StateContext, StateId, StateMusic};

use super::confirm::{self, ConfirmState};
use super::options::GameOptions;
use super::saves::{Mode, SavesState, age, slot_name};
use super::settings::SettingsState;
use super::{
    AUTOSAVES, BACK, Difficulty, GameInfo, Menu, RunStart, RunState, Shell, SlotState, activated,
    cancelled, enter, idle, load, newest, push, quit_game, read_slots, stack, start_run, theme,
    toast,
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

/// What the title shows beyond the game's info: the newest readable slot
/// and when it was written, and whether the picker is open. A resource
/// [`TitleState`] owns; absent, no slot and the main page.
#[derive(Debug, Clone, Copy, Default)]
pub struct TitleView {
    newest: Option<(&'static str, u64)>,
    picking: bool,
}

fn view(world: &World) -> TitleView {
    world
        .get_resource::<TitleView>()
        .copied()
        .unwrap_or_default()
}

/// The title's bindings and its picker's cards, which
/// `assets/ui/screens/title.json` names.
pub(super) fn bind(bindings: &mut UiBindings) {
    bindings
        .text("game_title", |w| w.resource::<GameInfo>().title.clone())
        .text("version", |w| {
            format!("Version {}", w.resource::<GameInfo>().version)
        })
        .visible("title_main", |w| !view(w).picking)
        .visible("title_picker", |w| view(w).picking)
        .enabled("has_newest", |w| view(w).newest.is_some())
        .visible("has_newest", |w| view(w).newest.is_some())
        .text("newest_slot", |w| {
            view(w)
                .newest
                .map_or_else(String::new, |(slot, _)| format!("{} ·", slot_name(slot)))
        })
        .text("newest_age", |w| {
            view(w).newest.map_or_else(String::new, |(_, at)| age(at))
        })
        .mount("difficulty_cards", |world, row| {
            let tree = world.resource_mut::<UiTree>();
            for at in 0..Difficulty::ALL.len() {
                difficulty_card(tree, row, at);
            }
        });
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
    root: Option<WidgetId>,
    newest: Option<(&'static str, SaveGame)>,
    autosave_ready: bool,
    opened: Option<WidgetId>,
}

impl TitleState {
    /// Shows the picker, or the main page, focused on Normal or New Game.
    fn show_picker(&self, world: &mut World, picking: bool) {
        let Some(root) = self.root else {
            return;
        };
        world.resource_mut::<TitleView>().picking = picking;
        screen_build::refresh(world, root);
        let key = if picking { PICKS[1] } else { "new_game" };
        if let Some(button) = screen_build::node(world, root, key) {
            world.resource_mut::<UiTree>().focus(button);
        }
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
        let newest = self.newest.as_ref().map(|(slot, _)| {
            let at = slots.iter().find_map(|(each, state)| match state {
                SlotState::Ready(_, at) if each == slot => Some(*at),
                _ => None,
            });
            (*slot, at.unwrap_or(0))
        });
        let picking = false;
        world.insert_resource(TitleView { newest, picking });
        stack(world).own_resource::<TitleView>();
        let root = Menu::build(world, "title").root;
        self.root = Some(root);
        self.show_picker(world, false);
        let first = screen_build::node(world, root, "continue");
        if let Some(first) = first.filter(|_| newest.is_some()) {
            world.resource_mut::<UiTree>().focus(first);
        }
        enter(world, root);
        let shell = world.resource_mut::<Shell>();
        if warnings > 0 && !std::mem::replace(&mut shell.warned, true) {
            let files = if warnings == 1 { "file" } else { "files" };
            toast::toast(world, format!("{warnings} save {files} could not be read"));
        }
    }

    fn on_exit(&mut self, _ctx: &mut StateContext) {}

    /// The title's song, which a screen over it, Settings or Load, keeps.
    fn music(&self) -> Option<StateMusic> {
        Some(StateMusic::new(theme::TITLE_MUSIC, theme::TITLE_FADE))
    }

    fn on_resume(&mut self, ctx: &mut StateContext) {
        if let Some(button) = self.opened {
            ctx.world.resource_mut::<UiTree>().focus(button);
        }
    }

    fn update(&mut self, world: &mut World) {
        let Some(root) = self.root.filter(|_| idle(world)) else {
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
            (Some(BACK), _) => self.show_picker(world, false),
            (None, _) if view(world).picking && cancelled(world, root) => {
                self.show_picker(world, false);
            }
            (Some(CONTINUE), _) => {
                if let Some((slot, save)) = self.newest.clone() {
                    load(world, slot, save);
                }
            }
            (Some(NEW_GAME), _) => self.show_picker(world, true),
            (Some(LOAD), _) => push(world, SavesState::new(Mode::Load)),
            (Some(SETTINGS), _) => push(world, SettingsState::new(0)),
            (Some(QUIT), _) => quit_game(world),
            _ => {}
        }
    }
}
