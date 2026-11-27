//! The story's screens (redo 8.1, P8-1): a dialogue box over the kit's
//! running conversation, built from a game's `ui_screens` file `dialogue`,
//! its line revealed a character at a time without relayout.

use glam::Vec2;
use tungsten_core::ui::{
    LayoutStyle, UiAction, UiBindings, UiEvent, UiReveals, UiRow, UiScreenRegistry, UiTree,
    Visibility, WidgetId, screen_build,
};
use tungsten_core::{
    AssetRegistry, DataRegistry, Entity, EventQueue, Plugin, Schedule, SpriteAssetId, Stage, Time,
    World, system,
};

use crate::kit::{Dialogue, Quest, QuestStatus, Quests, Talk};
use crate::screens::{self, ModalScreen};
use crate::state::{GameState, StateContext, StateId};

/// The dialogue box's screen ID and the `ui_screens` file it builds.
pub const DIALOGUE_SCREEN: StateId = "dialogue";
/// [`StoryUiPlugin`]'s system, in `update`: opens the box over a talk.
pub const DIALOGUE_OPEN: &str = "dialogue_open";

/// The journal's screen ID and the `ui_screens` file [`open_journal`]
/// opens.
pub const JOURNAL_SCREEN: &str = "journal";

/// The key of the box file's node that reveals: its line.
pub const TEXT_KEY: &str = "text";

/// The size a portrait takes when its sprite has none registered.
const PORTRAIT_SIZE: Vec2 = Vec2::new(64.0, 64.0);

/// Opts a game into the dialogue box (redo 8.1): registers the bindings a
/// `ui_screens` file `dialogue` names, the screen `dialogue`
/// ([`screens::register`]) and [`DIALOGUE_OPEN`] in `update`, which opens
/// the box while the kit's [`Dialogue`] runs a talk and no box is up. A
/// game without the file is warned once and keeps the talk for its own
/// Rust.
///
/// The file's root is modal on the popup layer (a `screen`, or a `root`
/// with `"layer": "game_popup", "modal": true`). Its bindings: text
/// `dialogue.speaker` and `dialogue.text` (the line, on the node keyed
/// `text`, which reveals); visible `dialogue.reading` and
/// `dialogue.choosing`; rows `dialogue.choices`, one a shown choice, each
/// row's `text` field its text; mount `dialogue.portrait`, an image the box
/// sets to each line's portrait and collapses without one; actions
/// `dialogue.continue` (on a button: shows the rest of a revealing line,
/// else advances) and `dialogue.choose` (on a choice row in the repeat:
/// takes the row's choice). Each line reveals from its first character at
/// its speaker's pace on the real clock; Cancel takes the node's `back`
/// choice when it is offered, else does nothing; the talk's end closes the
/// box. Whether play stops under it is the covered state's `holds_clock`.
///
/// The journal (redo 8.1, P8-2), a `ui_screens` file `journal` that
/// [`open_journal`] opens as a data screen, reads rows `quests.active`
/// (fields `title`, `journal`, the stage's line, and `objectives`, a line
/// an objective, "Lamp lit 0/1") and `quests.done` (`title`), from the
/// holder [`JournalView`] names, else the game's quests.
#[derive(Debug, Default, Clone, Copy)]
pub struct StoryUiPlugin;

impl Plugin for StoryUiPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        if !world.has_resource::<DialogueBoxView>() {
            world.insert_resource(DialogueBoxView::default());
        }
        UiBindings::of(world)
            .text("dialogue.speaker", speaker_text)
            .text("dialogue.text", line_text)
            .visible("dialogue.reading", reading)
            .visible("dialogue.choosing", choosing)
            .rows("dialogue.choices", choice_rows)
            .mount("dialogue.portrait", portrait_mount)
            .action("dialogue.continue", continue_action)
            .action("dialogue.choose", choose_action)
            .rows("quests.active", active_rows)
            .rows("quests.done", done_rows);
        if !world.has_resource::<JournalView>() {
            world.insert_resource(JournalView::default());
        }
        screens::register(world, DIALOGUE_SCREEN, make_box);
        schedule.add(Stage::Update, system(DIALOGUE_OPEN, dialogue_open));
    }
}

/// What the box shows of the talk, a World resource [`StoryUiPlugin`]
/// inserts: whether a box is up, the line it reveals and how far, its
/// portrait image.
#[derive(Debug, Default)]
pub struct DialogueBoxView {
    up: bool,
    warned: bool,
    /// The revealed line: its node and text.
    line: Option<(String, String)>,
    /// Characters shown, fractional as the pace climbs.
    shown: f32,
    /// The line's characters.
    total: u32,
    /// The portrait mount's image.
    portrait: Option<WidgetId>,
}

impl DialogueBoxView {
    /// Whether a dialogue box is up.
    #[must_use]
    pub fn up(&self) -> bool {
        self.up
    }

    /// The characters of the line shown now, while it reveals; `None` once
    /// it shows whole or none shows.
    #[must_use]
    pub fn revealing(&self) -> Option<u32> {
        (self.line.is_some() && (self.shown as u32) < self.total).then_some(self.shown as u32)
    }

    /// The portrait image the mount made.
    #[must_use]
    pub fn portrait(&self) -> Option<WidgetId> {
        self.portrait
    }
}

fn talk(world: &World) -> Option<&Talk> {
    world.get_resource::<Dialogue>()?.current()
}

fn speaker_text(world: &World) -> String {
    talk(world)
        .and_then(Talk::line)
        .map(|line| line.name().to_owned())
        .unwrap_or_default()
}

fn line_text(world: &World) -> String {
    talk(world)
        .and_then(Talk::line)
        .map(|line| line.text().to_owned())
        .unwrap_or_default()
}

fn reading(world: &World) -> bool {
    talk(world).is_some_and(|talk| talk.line().is_some())
}

fn choosing(world: &World) -> bool {
    talk(world).is_some_and(Talk::choosing)
}

fn choice_rows(world: &World) -> Vec<UiRow> {
    talk(world).map_or_else(Vec::new, |talk| {
        talk.choices()
            .map(|(index, text)| UiRow::new(index.to_string()).with("text", text))
            .collect()
    })
}

/// Fills the portrait mount with an image, collapsed until a line shows a
/// portrait.
fn portrait_mount(world: &mut World, node: WidgetId) {
    let tree = world.resource_mut::<UiTree>();
    let image = tree.image(
        node,
        SpriteAssetId::new(0),
        PORTRAIT_SIZE,
        LayoutStyle::default(),
    );
    tree.set_visibility(image, Visibility::Collapsed);
    world.resource_mut::<DialogueBoxView>().portrait = Some(image);
}

/// `dialogue.continue`: the rest of a revealing line, else the next.
fn continue_action(world: &mut World, _: UiAction) {
    let view = world.resource_mut::<DialogueBoxView>();
    if view.revealing().is_some() {
        view.shown = view.total as f32;
        return;
    }
    view.line = None;
    Dialogue::advance(world);
}

/// `dialogue.choose`: the row's choice.
fn choose_action(world: &mut World, call: UiAction) {
    let Some(index) = call.row.and_then(|row| row.parse::<usize>().ok()) else {
        return;
    };
    world.resource_mut::<DialogueBoxView>().line = None;
    Dialogue::choose(world, index);
}

/// Opens the box over a running talk while none is up.
fn dialogue_open(world: &mut World) {
    if talk(world).is_none() || !world.has_resource::<DialogueBoxView>() {
        return;
    }
    if world.resource::<DialogueBoxView>().up {
        return;
    }
    let has_file = world
        .get_resource::<UiScreenRegistry>()
        .is_some_and(|registry| registry.contains(DIALOGUE_SCREEN));
    if !has_file {
        let view = world.resource_mut::<DialogueBoxView>();
        if !std::mem::replace(&mut view.warned, true) {
            log::warn!(
                "dialogue: a talk runs and no `ui_screens` file `dialogue` is loaded; no box opens"
            );
        }
        return;
    }
    if screens::idle(world) {
        screens::open(world, DIALOGUE_SCREEN);
    }
}

fn make_box() -> Box<dyn GameState> {
    Box::new(DialogueBox::default())
}

/// The dialogue box: the file `dialogue` built and adopted as a modal
/// screen, its actions run, its line revealed and its bindings refreshed
/// each frame.
#[derive(Default)]
struct DialogueBox {
    screen: Option<ModalScreen>,
    closing: bool,
    /// Whether choices showed at the last frame.
    choosing: bool,
}

impl GameState for DialogueBox {
    fn id(&self) -> StateId {
        DIALOGUE_SCREEN
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        let world = &mut *ctx.world;
        let Some(built) = screen_build::build(world, DIALOGUE_SCREEN) else {
            world
                .resource_mut::<crate::state::StateStack>()
                .request_pop();
            return;
        };
        let screen = ModalScreen::adopt(world, built.root, built.card);
        let view = world.resource_mut::<DialogueBoxView>();
        view.up = true;
        view.line = None;
        self.choosing = choosing(world);
        screen.resume(world);
        self.screen = Some(screen);
    }

    fn on_exit(&mut self, ctx: &mut StateContext) {
        let world = &mut *ctx.world;
        if let Some(screen) = self.screen
            && let Some(text) = screen_build::node(world, screen.root, TEXT_KEY)
        {
            UiReveals::of(world).set(text, None);
        }
        let view = world.resource_mut::<DialogueBoxView>();
        view.up = false;
        view.line = None;
        view.portrait = None;
    }

    fn on_resume(&mut self, ctx: &mut StateContext) {
        if let Some(screen) = self.screen {
            screen.resume(ctx.world);
        }
    }

    fn update(&mut self, world: &mut World) {
        let Some(mut screen) = self.screen else {
            return;
        };
        if talk(world).is_none() {
            if !self.closing {
                self.closing = screens::pop(world);
            }
            screen.update(world);
            self.screen = Some(screen);
            return;
        }
        if screens::idle(world) {
            run_actions(world, screen.root);
            cancel(world, screen.root);
        }
        screen_build::refresh(world, screen.root);
        reveal(world, screen.root);
        show_portrait(world);
        let now = choosing(world);
        if now != self.choosing {
            self.choosing = now;
            screen.resume(world);
        }
        self.screen = Some(screen);
    }
}

/// Runs the named action of each control on `root` activated at this
/// routing.
fn run_actions(world: &mut World, root: WidgetId) {
    let activated: Vec<WidgetId> = {
        let tree = world.resource::<UiTree>();
        world
            .resource::<EventQueue<UiEvent>>()
            .iter_current()
            .filter_map(|event| match *event {
                UiEvent::Activated { widget, .. } if tree.root_of(widget) == Some(root) => {
                    Some(widget)
                }
                _ => None,
            })
            .collect()
    };
    for widget in activated {
        let Some(keyed) = screen_build::action(world, root, widget) else {
            continue;
        };
        let tungsten_core::ui::ScreenAction::Named(name) = &keyed.action else {
            continue;
        };
        let Some(action) = world
            .get_resource::<UiBindings>()
            .and_then(|bindings| bindings.action_named(name))
        else {
            continue;
        };
        action(
            world,
            UiAction {
                key: keyed.key.clone(),
                row: keyed.row.clone(),
                value: None,
            },
        );
    }
}

/// A `Cancel` on `root` takes the node's `back` choice when it is offered.
fn cancel(world: &mut World, root: WidgetId) {
    let cancelled = world
        .resource::<EventQueue<UiEvent>>()
        .iter_current()
        .any(|event| matches!(event, UiEvent::Cancel { scope, .. } if *scope == root));
    if !cancelled {
        return;
    }
    if let Some(back) = talk(world).and_then(Talk::back) {
        world.resource_mut::<DialogueBoxView>().line = None;
        Dialogue::choose(world, back);
    }
}

/// Reveals the shown line from its first character at its speaker's pace
/// on the real clock; a new line starts again at 0.
fn reveal(world: &mut World, root: WidgetId) {
    let Some(text) = screen_build::node(world, root, TEXT_KEY) else {
        return;
    };
    let line = talk(world).and_then(|talk| {
        let line = talk.line()?;
        Some((
            (talk.node().to_owned(), line.text().to_owned()),
            line.pace(),
            line.text().chars().count() as u32,
        ))
    });
    let dt = world.get_resource::<Time>().map_or(0.0, Time::real_delta);
    let view = world.resource_mut::<DialogueBoxView>();
    let count = match line {
        None => {
            view.line = None;
            None
        }
        Some((identity, pace, total)) => {
            if view.line.as_ref() != Some(&identity) {
                view.line = Some(identity);
                view.shown = 0.0;
                view.total = total;
            } else if view.shown < total as f32 {
                view.shown = (view.shown + pace * dt).min(total as f32);
            }
            view.revealing()
        }
    };
    UiReveals::of(world).set(text, count);
}

/// The portrait image shows the line's portrait, collapsed without one.
fn show_portrait(world: &mut World) {
    let Some(image) = world.resource::<DialogueBoxView>().portrait else {
        return;
    };
    let sprite = talk(world)
        .and_then(Talk::line)
        .and_then(|line| line.portrait())
        .and_then(|name| {
            let assets = world.get_resource::<AssetRegistry>()?;
            let id = assets.sprite_id(name)?;
            let size = assets
                .sprite_size(id)
                .map_or(PORTRAIT_SIZE, |(w, h)| Vec2::new(w as f32, h as f32));
            Some((id, size))
        });
    let tree = world.resource_mut::<UiTree>();
    match sprite {
        Some((id, size)) => {
            tree.set_image(image, id, size);
            tree.set_visibility(image, Visibility::Visible);
        }
        None => tree.set_visibility(image, Visibility::Collapsed),
    }
}

/// Whose quests the journal shows, a World resource [`StoryUiPlugin`]
/// inserts: a holder's, or the game's while none is named.
#[derive(Debug, Default)]
pub struct JournalView {
    holder: Option<Entity>,
}

impl JournalView {
    /// The holder the journal reads; `None` for the game's quests.
    #[must_use]
    pub fn holder(&self) -> Option<Entity> {
        self.holder
    }
}

/// Opens the journal over `holder`'s quests, `None` for the game's: sets
/// [`JournalView`] and opens the game's `ui_screens` file `journal` as a
/// data screen ([`screens::open`]); `false` when it does not open.
pub fn open_journal(world: &mut World, holder: Option<Entity>) -> bool {
    if !world.has_resource::<JournalView>() {
        world.insert_resource(JournalView::default());
    }
    world.resource_mut::<JournalView>().holder = holder;
    screens::open(world, JOURNAL_SCREEN)
}

/// The journal's holder's quests and each one's data, in ID order, with
/// the status asked for.
fn journal_quests(world: &World, status: QuestStatus) -> Vec<(&str, &Quest, &str)> {
    let holder = world
        .get_resource::<JournalView>()
        .and_then(JournalView::holder);
    let quests = match holder {
        Some(entity) => world.get::<Quests>(entity),
        None => world.get_resource::<Quests>(),
    };
    let (Some(quests), Some(data)) = (quests, world.get_resource::<DataRegistry>()) else {
        return Vec::new();
    };
    quests
        .iter()
        .filter(|(_, state)| state.status() == status)
        .filter_map(|(id, state)| Some((id, data.get::<Quest>(id)?, state.stage())))
        .collect()
}

fn active_rows(world: &World) -> Vec<UiRow> {
    let holder = world
        .get_resource::<JournalView>()
        .and_then(JournalView::holder);
    journal_quests(world, QuestStatus::Active)
        .into_iter()
        .map(|(id, quest, stage)| {
            let objectives: Vec<String> = quest
                .progress(world, holder, stage)
                .map(|(text, have, need)| format!("{text} {have}/{need}"))
                .collect();
            UiRow::new(id)
                .with("title", quest.title())
                .with("journal", quest.journal(stage).unwrap_or_default())
                .with("objectives", objectives.join("\n"))
        })
        .collect()
}

fn done_rows(world: &World) -> Vec<UiRow> {
    journal_quests(world, QuestStatus::Done)
        .into_iter()
        .map(|(id, quest, _)| UiRow::new(id).with("title", quest.title()))
        .collect()
}

#[cfg(test)]
#[path = "tests/story.rs"]
mod tests;
