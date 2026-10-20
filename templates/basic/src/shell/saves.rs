//! The save and load screen over the autosave and ten slots, in a card that
//! scrolls. Each row names its slot's state in text, in the mono family:
//! empty, ready (when, playtime, version and the game's summary), saved by a
//! newer version, or damaged; a newer or damaged slot is neither loaded nor
//! overwritten, and any slot with a file can be deleted after a confirm, a
//! delete that fails saying so. Save mode (from pause) saves to an empty
//! slot at once and asks before it overwrites; the autosave is no target.
//! Load mode (from the title, pause and game over) starts the run from a
//! ready slot, asking first only over a live run that is not checkpointed.

use std::time::{SystemTime, UNIX_EPOCH};

use tungsten::core::save::SaveError;
use tungsten::core::ui::{Sizing, UiTree, WidgetId};
use tungsten::core::{SaveSlots, World};
use tungsten::{GameState, StateContext, StateId};

use super::confirm::{self, ConfirmState};
use super::{
    AUTOSAVE, RunStart, SLOTS, Shell, SlotState, cancelled, clicked, enter, idle, pop, read_slots,
    run, save_to, start_run, theme, toast, widgets,
};
use crate::save;

pub const SAVES: StateId = "saves";

/// What the screen does with a slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Save,
    Load,
}

/// The save and load screen.
pub struct SavesState {
    mode: Mode,
    /// The root, each slot's button and delete button, and Back.
    ui: Option<(WidgetId, Vec<[WidgetId; 2]>, WidgetId)>,
    states: Vec<SlotState>,
    opened: Option<WidgetId>,
}

impl SavesState {
    #[must_use]
    pub fn new(mode: Mode) -> Self {
        let (ui, states, opened) = (None, Vec::new(), None);
        Self {
            mode,
            ui,
            states,
            opened,
        }
    }

    /// Reads the slots again, rewrites the rows and focuses the button that
    /// opened the screen above, else the first slot that can take focus.
    fn refresh(&mut self, world: &mut World) {
        let (slots, _) = read_slots(world);
        let Some((_, rows, back)) = &self.ui else {
            return;
        };
        let tree = world.resource_mut::<UiTree>();
        for ((slot, state), [button, delete]) in slots.iter().zip(rows) {
            let name = slot_name(slot);
            let (text, usable) = match state {
                SlotState::Off => (format!("{name} · Saving is off"), false),
                SlotState::Empty => (format!("{name} · Empty"), self.mode == Mode::Save),
                SlotState::Ready(save, at) => {
                    let (meta, ago) = (&save.meta, age(*at));
                    let played = clock(meta.playtime_secs);
                    let summary = save::summary(save);
                    let version = &meta.game_version;
                    let line = format!("{name} · {ago} · {played} played · v{version}\n{summary}");
                    (line, true)
                }
                SlotState::Newer => (format!("{name} · Saved by a newer version"), false),
                SlotState::Damaged => (format!("{name} · Damaged"), false),
            };
            let target = self.mode == Mode::Load || *slot != AUTOSAVE;
            let text = match target {
                true => text,
                false => format!("{name} · Written when you pause or quit"),
            };
            tree.set_text(*button, text.as_str());
            tree.set_enabled(*button, usable && target);
            widgets::show(
                tree,
                *delete,
                !matches!(state, SlotState::Off | SlotState::Empty),
            );
        }
        let slots_first = rows.iter().map(|[button, _]| *button);
        widgets::focus_first(
            tree,
            self.opened.into_iter().chain(slots_first).chain([*back]),
        );
        self.states = slots.into_iter().map(|(_, state)| state).collect();
    }

    /// The slot at `at` activated.
    fn pick(&mut self, world: &mut World, at: usize) {
        let slot = SLOTS[at];
        let name = slot_name(slot);
        match (&self.states[at], self.mode) {
            (SlotState::Empty, Mode::Save) => {
                saved(world, slot);
                self.refresh(world);
            }
            (SlotState::Ready(..), Mode::Save) => {
                let ask = ConfirmState::new(&format!("Overwrite {name}?"), "Overwrite", "Cancel");
                confirm::ask(world, ask.on_yes(move |world| saved(world, slot)));
            }
            (SlotState::Ready(save, _), Mode::Load) => {
                let start = RunStart::from(save.as_ref().clone());
                let checkpointed = run(world).is_some_and(|run| run.checkpointed);
                if save::capture(world).is_none() || checkpointed {
                    return start_run(world, start, Some(slot));
                }
                let question = format!("Load {name}? Progress since your last save will be lost.");
                let ask = ConfirmState::new(&question, "Load", "Cancel");
                confirm::ask(
                    world,
                    ask.on_yes(move |world| start_run(world, start, Some(slot))),
                );
            }
            _ => {}
        }
    }
}

/// Saves the run to `slot` and says how it went.
fn saved(world: &mut World, slot: &'static str) {
    let name = slot_name(slot);
    let text = match save_to(world, slot) {
        true => format!("Saved to {name}"),
        false => format!("{name} could not be saved"),
    };
    toast::toast(world, text);
}

/// Deletes `slot`'s file, so it can be written again; a file that stays
/// keeps the slot read-only, and a toast says so.
fn delete(world: &mut World, slot: &'static str) {
    match world.resource::<SaveSlots>().delete(slot) {
        Ok(()) | Err(SaveError::NotFound { .. }) => {
            let shell = world.resource_mut::<Shell>();
            shell.read_only.retain(|kept| *kept != slot);
        }
        Err(error) => {
            log::warn!("Save slot '{slot}' not deleted: {error}");
            toast::toast(world, format!("{} could not be deleted", slot_name(slot)));
        }
    }
}

/// A slot's name on screen.
pub(crate) fn slot_name(slot: &str) -> String {
    match slot.strip_prefix("slot_") {
        Some(number) => format!("Slot {number}"),
        None => "Autosave".to_owned(),
    }
}

/// How long ago a save written at `at` (Unix ms) was, roughly.
pub(crate) fn age(at: u64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let now = u64::try_from(now.as_millis()).unwrap_or(u64::MAX);
    match now.saturating_sub(at) / 60_000 {
        0 => "just now".to_owned(),
        minutes @ 1..60 => format!("{minutes} min ago"),
        minutes @ 60..1440 => format!("{} h ago", minutes / 60),
        minutes => format!("{} days ago", minutes / 1440),
    }
}

/// Seconds as `m:ss`, or `h:mm:ss` past an hour.
fn clock(secs: f64) -> String {
    let secs = secs.max(0.0) as u64;
    let (hours, minutes, seconds) = (secs / 3600, secs / 60 % 60, secs % 60);
    match hours {
        0 => format!("{minutes}:{seconds:02}"),
        _ => format!("{hours}:{minutes:02}:{seconds:02}"),
    }
}

impl GameState for SavesState {
    fn id(&self) -> StateId {
        SAVES
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        let tree = ctx.world.resource_mut::<UiTree>();
        let (root, card) = widgets::screen(tree, theme::WIDE_WIDTH);
        let heading = ["Load", "Save"][usize::from(self.mode == Mode::Save)];
        widgets::heading(tree, card, heading, false);
        let mut rows = Vec::new();
        for slot in SLOTS {
            let (row, name) = (widgets::row(tree, card), slot_name(slot));
            let button = widgets::button(tree, row, &name);
            widgets::set_width(tree, button, Sizing::Fill);
            widgets::mono(tree, button);
            let delete = widgets::button(tree, row, &format!("Delete {name}"));
            tree.set_text(delete, "Delete");
            widgets::danger(tree, delete);
            rows.push([button, delete]);
        }
        let back = widgets::button(tree, card, "Back");
        enter(ctx.world, root);
        self.ui = Some((root, rows, back));
        self.refresh(ctx.world);
    }

    fn on_exit(&mut self, _ctx: &mut StateContext) {}

    fn on_resume(&mut self, ctx: &mut StateContext) {
        self.refresh(ctx.world);
    }

    fn update(&mut self, world: &mut World) {
        let Some((root, rows, back)) = self.ui.clone().filter(|_| idle(world)) else {
            return;
        };
        let buttons: Vec<WidgetId> = rows.iter().flatten().copied().chain([back]).collect();
        let pick = clicked(world, &buttons);
        self.opened = pick.map(|at| buttons[at]).or(self.opened);
        match pick {
            Some(at) if at == buttons.len() - 1 => pop(world),
            Some(at) if at % 2 == 0 => self.pick(world, at / 2),
            Some(at) => {
                let slot = SLOTS[at / 2];
                let question = format!("Delete {}?", slot_name(slot));
                let ask = ConfirmState::new(&question, "Delete", "Cancel");
                confirm::ask(world, ask.on_yes(move |world| delete(world, slot)));
            }
            None if cancelled(world, root) => pop(world),
            None => {}
        }
    }
}
