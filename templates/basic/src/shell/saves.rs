//! The save and load screen in a card that scrolls: the autosaves, newest
//! first, under a small "Autosaves" label, then the ten slots under
//! "Slots". Each row's button holds the slot's name and, smaller, muted and
//! in mono, its details: when, playtime, an older version's number and the
//! game's summary, or "Empty", "Saved by a newer version" or "Damaged". An
//! empty row reads quieter; a newer or damaged slot is neither loaded nor
//! overwritten, and any slot with a file can be deleted after a confirm, a
//! delete that fails saying so; Delete stays muted until its row has focus.
//! Save mode (from pause) saves to an empty slot at once and asks before it
//! overwrites; it shows no autosave, only a note on when they are written.
//! Load mode (from the title, pause and game over) starts the run from a
//! ready slot, asking first over an older version's save or a live run that
//! is not checkpointed.

use std::time::{SystemTime, UNIX_EPOCH};

use tungsten::core::save::SaveError;
use tungsten::core::text::{TextAlign, TextWrap};
use tungsten::core::ui::{Align, Direction, Justify, Sizing, UiTree, WidgetId};
use tungsten::core::{SaveSlots, World};
use tungsten::{GameState, StateContext, StateId};

use super::confirm::{self, ConfirmState};
use super::options::GameOptions;
use super::{
    AUTOSAVES, SLOTS, Shell, SlotState, cancelled, clicked, enter, idle, load, pop, read_slots,
    save_to, theme, toast, version_note, widgets,
};
use crate::save;

pub const SAVES: StateId = "saves";
/// The rows under "Autosaves": the three and the old shell's one.
const AUTOSAVE_ROWS: usize = AUTOSAVES.len() + 1;

/// What the screen does with a slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Save,
    Load,
}

/// The save and load screen.
pub struct SavesState {
    mode: Mode,
    /// The root, each row's button and Delete, the autosaves' rows first,
    /// the note that stands for them in Save mode, and Back.
    ui: Option<(WidgetId, Vec<[WidgetId; 2]>, WidgetId, WidgetId)>,
    /// Each row's slot and state, as the last refresh drew them; `None`
    /// for a hidden row.
    rows: Vec<Option<(&'static str, SlotState)>>,
    opened: Option<WidgetId>,
}

impl SavesState {
    #[must_use]
    pub fn new(mode: Mode) -> Self {
        let (ui, rows, opened) = (None, Vec::new(), None);
        Self {
            mode,
            ui,
            rows,
            opened,
        }
    }

    /// Reads the slots again, the autosaves newest first and none in Save
    /// mode, rewrites the rows and focuses the button that opened the
    /// screen above, else the first slot that can take focus.
    fn refresh(&mut self, world: &mut World) {
        let (slots, _) = read_slots(world);
        let (mut autosaves, slots): (Vec<_>, Vec<_>) =
            slots.into_iter().partition(|(slot, _)| autosave(slot));
        autosaves.sort_by_key(|(_, state)| match state {
            SlotState::Ready(_, at) => std::cmp::Reverse(Some(*at)),
            _ => std::cmp::Reverse(None),
        });
        let saving = self.mode == Mode::Save;
        let mut shown: Vec<_> = autosaves
            .into_iter()
            .filter(|_| !saving)
            .map(Some)
            .collect();
        shown.resize_with(AUTOSAVE_ROWS, || None);
        shown.extend(slots.into_iter().map(Some));
        let details: Vec<Option<String>> = shown
            .iter()
            .map(|row| row.as_ref().map(|(_, state)| details(world, state)))
            .collect();
        let on = world.resource::<GameOptions>().autosave;
        let Some((_, rows, note, back)) = &self.ui else {
            return;
        };
        let tree = world.resource_mut::<UiTree>();
        let colors = tree.theme().tokens.colors;
        let written = "Written as you play, when you pause and when you quit.";
        tree.set_text(*note, [written, "Autosave is off."][usize::from(!on)]);
        widgets::show(tree, *note, saving);
        for (([button, delete], row), text) in rows.iter().zip(&shown).zip(&details) {
            if let Some(parent) = tree.parent(*button) {
                widgets::show(tree, parent, row.is_some());
            }
            let (Some((slot, state)), Some(text)) = (row, text) else {
                continue;
            };
            let name = slot_name(slot);
            widgets::name(tree, *button, &name);
            widgets::name(tree, *delete, &format!("Delete {name}"));
            tree.set_text(*button, name.as_str());
            let empty = matches!(state, SlotState::Empty);
            let usable = matches!(state, SlotState::Ready(..)) || (empty && saving);
            tree.set_enabled(*button, usable);
            // The details as quiet as the row: muted, or disabled with it.
            if let Some(&label) = tree.children(*button).get(1) {
                tree.set_text(label, text.as_str());
                let color = [colors.text_muted, colors.disabled_text][usize::from(!usable)];
                widgets::restyle(tree, label, |text| text.color = Some(color));
            }
            // An empty row has no fill; enabled, its name is muted too.
            let mut style = tree.overrides(*button).cloned().unwrap_or_default();
            style.background = empty.then_some([0; 4]);
            style.text.color = (empty && saving).then_some(colors.text_muted);
            tree.set_overrides(*button, style);
            widgets::show(tree, *delete, !matches!(state, SlotState::Off) && !empty);
        }
        let slots_first = rows.iter().map(|[button, _]| *button);
        widgets::focus_first(
            tree,
            self.opened.into_iter().chain(slots_first).chain([*back]),
        );
        self.rows = shown;
    }

    /// Each Delete in the muted colour, in danger while its row's button or
    /// the Delete itself has focus.
    fn quiet_deletes(&self, world: &mut World) {
        let Some((_, rows, ..)) = &self.ui else {
            return;
        };
        let tree = world.resource_mut::<UiTree>();
        let (colors, focused) = (tree.theme().tokens.colors, tree.focused());
        for [button, delete] in rows {
            let hot = focused == Some(*button) || focused == Some(*delete);
            let color = [colors.text_muted, colors.danger][usize::from(hot)];
            widgets::restyle(tree, *delete, |text| text.color = Some(color));
        }
    }

    /// The row at `at` activated.
    fn pick(&mut self, world: &mut World, at: usize) {
        let Some(Some((slot, state))) = self.rows.get(at) else {
            return;
        };
        let (slot, name) = (*slot, slot_name(slot));
        match (state, self.mode) {
            (SlotState::Empty, Mode::Save) => {
                saved(world, slot);
                self.refresh(world);
            }
            (SlotState::Ready(..), Mode::Save) => {
                let ask = ConfirmState::new(&format!("Overwrite {name}?"), "Overwrite", "Cancel");
                confirm::ask(world, ask.on_yes(move |world| saved(world, slot)));
            }
            (SlotState::Ready(save, _), Mode::Load) => load(world, slot, save.as_ref().clone()),
            _ => {}
        }
    }
}

/// A save row under `parent`: a button holding the slot's name and, beside
/// it at the end, smaller, muted and in mono, its details; then Delete, as
/// tall as the row.
fn save_row(tree: &mut UiTree, parent: WidgetId) -> [WidgetId; 2] {
    let row = widgets::row(tree, parent);
    let mut style = tree.layout_style(row).cloned().unwrap_or_default();
    style.align_items = Align::Stretch;
    tree.set_layout_style(row, style);
    let button = widgets::button(tree, row, "");
    let mut style = tree.layout_style(button).cloned().unwrap_or_default();
    (style.width, style.direction) = (Sizing::Fill, Direction::Row);
    (style.align_items, style.gap) = (Align::Center, 2.0 * theme::GAP);
    tree.set_layout_style(button, style);
    // The name on one line at its own width, the details in the rest: a
    // box sized to its text can commit an ulp short and wrap (known issues).
    let name = tree.children(button)[0];
    widgets::restyle(tree, name, |text| text.wrap = Some(TextWrap::None));
    let details = widgets::small(tree, button, "");
    widgets::set_width(tree, details, Sizing::Fill);
    widgets::mono(tree, details);
    widgets::restyle(tree, details, |text| text.align = Some(TextAlign::End));
    let delete = widgets::button(tree, row, "Delete");
    let mut style = tree.layout_style(delete).cloned().unwrap_or_default();
    style.justify_content = Justify::Center;
    tree.set_layout_style(delete, style);
    [button, delete]
}

/// A row's details: when, the playtime, an older version's number and the
/// game's summary; else the slot's state.
fn details(world: &World, state: &SlotState) -> String {
    match state {
        SlotState::Off => "Saving is off".to_owned(),
        SlotState::Empty => "Empty".to_owned(),
        SlotState::Ready(save, at) => {
            let note = version_note(world, &save.meta.game_version);
            let note = note.map_or(String::new(), |note| format!(" · {note}"));
            let (ago, played) = (age(*at), clock(save.meta.playtime_secs));
            format!("{ago} · {played} played{note}\n{}", save::summary(save))
        }
        SlotState::Older(_) => "Saved by an older version".to_owned(),
        SlotState::Newer => "Saved by a newer version".to_owned(),
        SlotState::Damaged => "Damaged".to_owned(),
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

/// A slot's name on screen: "Slot 1", "Autosave 1", or "Autosave" for the
/// old shell's one.
pub(crate) fn slot_name(slot: &str) -> String {
    match (slot.strip_prefix("slot_"), slot.strip_prefix("autosave_")) {
        (Some(number), _) => format!("Slot {number}"),
        (_, Some(number)) => format!("Autosave {number}"),
        _ => "Autosave".to_owned(),
    }
}

/// Whether `slot` is an autosave, the old shell's included.
fn autosave(slot: &str) -> bool {
    !slot.starts_with("slot_")
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
        // Rows for the autosaves and the old shell's, then the slots; the
        // refresh names them and hides what is left over.
        widgets::small(tree, card, "Autosaves");
        let note = widgets::note(tree, card, "");
        let mut rows: Vec<_> = (0..AUTOSAVE_ROWS).map(|_| save_row(tree, card)).collect();
        widgets::small(tree, card, "Slots");
        rows.extend((AUTOSAVES.len()..SLOTS.len()).map(|_| save_row(tree, card)));
        let back = widgets::button(tree, card, "Back");
        enter(ctx.world, root);
        self.ui = Some((root, rows, note, back));
        self.refresh(ctx.world);
        self.quiet_deletes(ctx.world);
    }

    fn on_exit(&mut self, _ctx: &mut StateContext) {}

    fn on_resume(&mut self, ctx: &mut StateContext) {
        self.refresh(ctx.world);
    }

    fn update(&mut self, world: &mut World) {
        self.quiet_deletes(world);
        let Some((root, rows, _, back)) = self.ui.clone().filter(|_| idle(world)) else {
            return;
        };
        let buttons: Vec<WidgetId> = rows.iter().flatten().copied().chain([back]).collect();
        let pick = clicked(world, &buttons);
        self.opened = pick.map(|at| buttons[at]).or(self.opened);
        match pick {
            Some(at) if at == buttons.len() - 1 => pop(world),
            Some(at) if at % 2 == 0 => self.pick(world, at / 2),
            Some(at) => {
                let Some(Some((slot, _))) = self.rows.get(at / 2) else {
                    return;
                };
                let slot = *slot;
                let question = format!("Delete {}?", slot_name(slot));
                let ask = ConfirmState::new(&question, "Delete", "Cancel");
                confirm::ask(world, ask.on_yes(move |world| delete(world, slot)));
            }
            None if cancelled(world, root) => pop(world),
            None => {}
        }
    }
}
