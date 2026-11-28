//! Keybindings: a row per action `input.json`'s `labels` lists, on one grid
//! under the card: its label, its Primary and Alternate key or mouse
//! binding and its Gamepad binding (`ActionMap::binding_at`), and a Reset,
//! each column as wide as its widest cell, so the cells line up; the card
//! scrolls only when the rows outgrow the viewport.
//! A cell captures the next press for 5 real seconds; a press of the other
//! device class is refused, and a binding another listed action holds asks
//! to swap. Each change persists through the action map's user layer;
//! Reset and Reset all return to the shipped bindings (`ActionMap::reset`).
//! The action map refuses a label for a `ui_*` or `engine_*` action, so the
//! menus cannot be lost.

use tungsten::core::ui::{Track, UiEvent, UiKey, UiRouter, UiTree, WidgetId, widgets};
use tungsten::core::{ActionMap, ActionMapError, Binding, BindingDevice, World};
use tungsten::screens::{self, Confirm, ModalScreen, toast};
use tungsten::{GameState, StateContext, StateId};

use super::events;

pub const KEYBINDINGS: StateId = "keybindings";
/// A row's binding columns.
pub const COLUMNS: [&str; 3] = ["Primary", "Alternate", "Gamepad"];
/// Real seconds a capture waits for a press.
pub const CAPTURE_SECS: f32 = 5.0;
const RESET_ALL: UiKey = UiKey::new("reset_all");
/// Each column's cell: the device and the index among the action's
/// bindings on it.
const CELLS: [(BindingDevice, usize); 3] = [
    (BindingDevice::Keyboard, 0),
    (BindingDevice::Keyboard, 1),
    (BindingDevice::Gamepad, 0),
];
/// The card's design width, past the smallest viewport's, so a stick's
/// name keeps one line there.
const WIDTH: f32 = 760.0;
/// The grid's columns: the captions, the three binding columns sharing
/// what is left, and the Resets.
const TRACKS: [Track; 5] = [
    Track::Auto,
    Track::Fr(1.0),
    Track::Fr(1.0),
    Track::Fr(1.0),
    Track::Auto,
];

/// The key of `action`'s cell in `column`, its Reset at column 3, as
/// `jump.primary`.
fn cell_key(action: &str, column: usize) -> String {
    let column = ["primary", "alternate", "gamepad", "reset"][column];
    format!("{action}.{column}")
}

/// Persists the action map's user layer; without a user folder the change
/// lasts the run.
fn persist(world: &mut World) {
    let Some(map) = world.get_resource_mut::<ActionMap>() else {
        return;
    };
    match map.persist() {
        Ok(()) | Err(ActionMapError::MissingSourcePath) => {}
        Err(error) => log::warn!("Bindings not saved: {error}"),
    }
}

/// Returns every action, or only `only`, to its shipped bindings, and
/// persists.
pub fn reset(world: &mut World, only: Option<&str>) {
    let Some(map) = world.get_resource_mut::<ActionMap>() else {
        return;
    };
    match only {
        Some(action) => map.reset(action),
        None => map.reset_all(),
    }
    persist(world);
}

/// The keybindings screen.
#[derive(Default)]
pub struct KeybindingsState {
    /// The listed actions, in `labels`' order, as the screen opened.
    actions: Vec<String>,
    /// The root and each row's three cells and Reset.
    ui: Option<(WidgetId, Vec<[WidgetId; 4]>)>,
    /// The row and column capturing, which the router times out.
    capture: Option<(usize, usize)>,
    /// The modal screen, which closes on a cancel or Back.
    screen: Option<ModalScreen>,
}

impl KeybindingsState {
    /// The row and column of the cell keyed `key`.
    fn cell_of(&self, key: UiKey) -> Option<(usize, usize)> {
        self.actions.iter().enumerate().find_map(|(row, action)| {
            let column = (0..4).find(|column| UiKey::new(&cell_key(action, *column)) == key);
            column.map(|column| (row, column))
        })
    }

    fn refresh(&self, world: &mut World) {
        let (Some((_, rows)), Some(map)) = (&self.ui, world.get_resource::<ActionMap>()) else {
            return;
        };
        let labels: Vec<Vec<Option<String>>> = self
            .actions
            .iter()
            .map(|action| {
                let cell = |(device, index)| map.binding_at(action, device, index);
                CELLS
                    .map(|at| cell(at).map(|binding| binding.label().into_owned()))
                    .to_vec()
            })
            .collect();
        let tree = world.resource_mut::<UiTree>();
        for (row, labels) in rows.iter().zip(labels) {
            for (id, label) in row.iter().zip(labels) {
                tree.set_text(*id, label.as_deref().unwrap_or("—"));
            }
        }
    }

    /// The capturing cell took `binding`.
    fn captured(&self, world: &mut World, row: usize, column: usize, binding: Binding) {
        let (device, index) = CELLS[column];
        if binding.device() != device {
            let wanted = ["keys", "keys", "pad buttons"][column];
            return toast(world, format!("{} takes {wanted}", COLUMNS[column]));
        }
        let action = self.actions[row].clone();
        let map = world.resource::<ActionMap>();
        let old = map.binding_at(&action, device, index);
        let holder = self
            .actions
            .iter()
            .find(|other| **other != action && map.bindings(other).contains(&binding));
        let Some(other) = holder.cloned() else {
            let map = world.resource_mut::<ActionMap>();
            map.set_binding_at(&action, device, index, binding);
            return persist(world);
        };
        let label = map.label(&other).unwrap_or(&other).to_owned();
        let swap = move |world: &mut World| {
            let map = world.resource_mut::<ActionMap>();
            let theirs = map.bindings(&other).iter();
            let theirs = theirs.filter_map(|each| if *each == binding { old } else { Some(*each) });
            map.replace_bindings(other.as_str(), theirs.collect());
            map.set_binding_at(&action, device, index, binding);
            persist(world);
        };
        let ask = Confirm::new(&format!("Already used by {label}. Swap?"), "Swap", "Cancel");
        screens::ask(world, ask.on_yes(swap));
    }

    fn stop_capture(&mut self, world: &mut World) {
        if self.capture.take().is_some() {
            world.resource_mut::<UiRouter>().cancel_capture();
        }
        self.refresh(world);
    }
}

impl GameState for KeybindingsState {
    fn id(&self) -> StateId {
        KEYBINDINGS
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        let map = ctx.world.get_resource::<ActionMap>();
        let labelled = map.map(|map| map.labelled().to_vec()).unwrap_or_default();
        self.actions = labelled.iter().map(|(action, _)| action.clone()).collect();
        let tree = ctx.world.resource_mut::<UiTree>();
        let (root, card) = widgets::screen(tree, WIDTH);
        widgets::heading(tree, card, "Keybindings", false);
        let grid = widgets::grid(tree, card, TRACKS);
        // The header: no caption, the columns' notes, no Reset.
        widgets::note(tree, grid, "");
        for column in COLUMNS {
            widgets::note(tree, grid, column);
        }
        widgets::note(tree, grid, "");
        let rows = labelled
            .iter()
            .map(|(action, label)| {
                tree.label(grid, label.as_str());
                let [a, b, c] =
                    COLUMNS.map(|column| widgets::button(tree, grid, &format!("{label} {column}")));
                let reset = widgets::button(tree, grid, &format!("Reset {label}"));
                tree.set_text(reset, "Reset");
                let cells = [a, b, c, reset];
                for (column, cell) in cells.into_iter().enumerate() {
                    tree.set_key(cell, &cell_key(action, column));
                }
                cells
            })
            .collect::<Vec<_>>();
        let actions = widgets::row(tree, card);
        let all = widgets::button(tree, actions, "Reset all");
        tree.set_key(all, "reset_all");
        let back = widgets::button(tree, actions, "Back");
        tree.set_key(back, "back");
        if let Some(first) = rows.first() {
            tree.focus(first[0]);
        }
        self.screen = Some(ModalScreen::adopt(ctx.world, root, card));
        self.ui = Some((root, rows));
        self.refresh(ctx.world);
    }

    fn on_exit(&mut self, ctx: &mut StateContext) {
        self.stop_capture(ctx.world);
    }

    fn on_resume(&mut self, ctx: &mut StateContext) {
        self.refresh(ctx.world);
    }

    fn update(&mut self, world: &mut World) {
        // A cancel or Back closes the screen there; nothing while capturing.
        let Some(screen) = self.screen.as_mut() else {
            return;
        };
        let chosen = screen.update(world);
        let Some((_, rows)) = self.ui.clone() else {
            return;
        };
        if let Some((row, column)) = self.capture {
            for event in events(world) {
                match event {
                    UiEvent::InputCaptured { binding, .. } => {
                        self.capture = None;
                        self.captured(world, row, column, binding);
                        return self.refresh(world);
                    }
                    UiEvent::CaptureCancelled { .. } => return self.stop_capture(world),
                    _ => {}
                }
            }
            let left = world.resource::<UiRouter>().capture_remaining();
            let left = left.unwrap_or_default();
            let wanted = if column == 2 { "button" } else { "key" };
            let text = format!("Press a {wanted}… ({} s)", left.ceil());
            let tree = world.resource_mut::<UiTree>();
            tree.set_text(rows[row][column], text.as_str());
            return;
        }
        let Some(key) = chosen else {
            return;
        };
        match (key, self.cell_of(key)) {
            (RESET_ALL, _) => {
                reset(world, None);
                self.refresh(world);
            }
            (_, Some((row, 3))) => {
                reset(world, Some(self.actions[row].as_str()));
                self.refresh(world);
            }
            (_, Some((row, column))) => {
                self.capture = Some((row, column));
                let router = world.resource_mut::<UiRouter>();
                router.capture_next_input_for(rows[row][column], CAPTURE_SECS);
            }
            (_, None) => {}
        }
    }
}
