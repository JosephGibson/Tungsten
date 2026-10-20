//! Keybindings: a row per `game::BINDABLE_ACTIONS` entry with its Primary
//! and Alternate key or mouse binding and its Gamepad binding, and a Reset.
//! A cell captures the next press for 5 real seconds; a press of the other
//! device class is refused, and a binding another bindable action holds
//! asks to swap. Each change persists through the action map's user layer;
//! Reset and Reset all return to `input.json`. The `ui_*` and `engine_*`
//! actions are never listed, so the menus cannot be lost.

use std::path::Path;

use tungsten::core::ui::{LayoutStyle, Sizing, UiEvent, UiRouter, UiTree, WidgetId};
use tungsten::core::{ActionMap, ActionMapError, Binding, Time, World};
use tungsten::{GameState, RootCover, StateContext, StateId};

use super::confirm::{self, ConfirmState};
use super::{
    cancelled, clicked, events, flow_pending, pop, stack, theme, toast, transitioning, widgets,
};
use crate::game::BINDABLE_ACTIONS;

pub const KEYBINDINGS: StateId = "keybindings";
/// A row's binding columns.
pub const COLUMNS: [&str; 3] = ["Primary", "Alternate", "Gamepad"];
/// Real seconds a capture waits for a press.
pub const CAPTURE_SECS: f32 = 5.0;
/// A binding cell's width, and the screen's.
const CELL: f32 = 110.0;
const WIDTH: f32 = theme::CAPTION_WIDTH + 4.0 * CELL + 5.0 * theme::GAP + 2.0 * theme::PADDING;

fn is_pad(binding: Binding) -> bool {
    matches!(
        binding,
        Binding::GamepadButton { .. } | Binding::GamepadAxis { .. }
    )
}

/// The binding in `column`: the first or second key or mouse binding, or
/// the first pad one.
fn cell(bindings: &[Binding], column: usize) -> Option<Binding> {
    let mut kind = bindings
        .iter()
        .filter(|each| is_pad(**each) == (column == 2));
    kind.nth(usize::from(column == 1)).copied()
}

/// `bindings` with `column`'s binding swapped for `with`, the rest kept.
fn with_cell(bindings: &[Binding], column: usize, with: Binding) -> Vec<Binding> {
    let mut out = bindings.to_vec();
    match cell(bindings, column).and_then(|old| out.iter().position(|each| *each == old)) {
        Some(at) => out[at] = with,
        None => out.push(with),
    }
    out
}

/// Replaces `action`'s bindings and persists them; without a user folder
/// the change lasts the run.
fn bind(world: &mut World, action: &str, bindings: Vec<Binding>) {
    let Some(map) = world.get_resource_mut::<ActionMap>() else {
        return;
    };
    match map.replace_bindings_and_persist(action, bindings) {
        Ok(()) | Err(ActionMapError::MissingSourcePath) => {}
        Err(error) => log::warn!("Bindings not saved: {error}"),
    }
}

/// Returns every bindable action, or only `only`, to `input.json`'s
/// bindings over the engine's defaults.
pub fn reset(world: &mut World, only: Option<&str>) {
    let shipped = ActionMap::load(Path::new("input.json")).map(ActionMap::merged_with_defaults);
    let Ok(shipped) = shipped else {
        return log::warn!("input.json not read, so the bindings stay");
    };
    for (action, _) in BINDABLE_ACTIONS
        .iter()
        .filter(|(action, _)| only.is_none_or(|only| only == *action))
    {
        bind(world, action, shipped.bindings(action).to_vec());
    }
}

/// The keybindings screen.
#[derive(Default)]
pub struct KeybindingsState {
    /// The root, each row's three cells and Reset, Reset all and Back.
    ui: Option<(WidgetId, Vec<[WidgetId; 4]>, WidgetId, WidgetId)>,
    /// The row and column capturing, and its real seconds left.
    capture: Option<(usize, usize, f32)>,
}

impl KeybindingsState {
    fn refresh(&self, world: &mut World) {
        let (Some((_, rows, ..)), Some(map)) = (&self.ui, world.get_resource::<ActionMap>()) else {
            return;
        };
        let bindings: Vec<Vec<Binding>> = BINDABLE_ACTIONS
            .iter()
            .map(|(action, _)| map.bindings(action).to_vec())
            .collect();
        let tree = world.resource_mut::<UiTree>();
        for (row, bindings) in rows.iter().zip(bindings) {
            for (column, id) in row.iter().take(3).enumerate() {
                let label = cell(&bindings, column).map(Binding::label);
                tree.set_text(*id, label.as_deref().unwrap_or("—"));
            }
        }
    }

    /// The capturing cell took `binding`.
    fn captured(&self, world: &mut World, row: usize, column: usize, binding: Binding) {
        if is_pad(binding) != (column == 2) {
            let wanted = ["keys", "keys", "pad buttons"][column];
            return toast::toast(world, format!("{} takes {wanted}", COLUMNS[column]));
        }
        let (action, _) = BINDABLE_ACTIONS[row];
        let map = world.resource::<ActionMap>();
        let mine = map.bindings(action).to_vec();
        let old = cell(&mine, column);
        let holder = BINDABLE_ACTIONS
            .iter()
            .find(|(other, _)| *other != action && map.bindings(other).contains(&binding));
        let Some(&(other, label)) = holder else {
            return bind(world, action, with_cell(&mine, column, binding));
        };
        let swap = move |world: &mut World| {
            let theirs = world.resource::<ActionMap>().bindings(other).to_vec();
            let theirs = theirs
                .into_iter()
                .filter_map(|each| if each == binding { old } else { Some(each) });
            bind(world, other, theirs.collect());
            bind(world, action, with_cell(&mine, column, binding));
        };
        let ask = ConfirmState::new(&format!("Already used by {label}. Swap?"), "Swap", "Cancel");
        confirm::ask(world, ask.on_yes(swap));
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
        let tree = ctx.world.resource_mut::<UiTree>();
        let root = widgets::screen(tree, WIDTH);
        widgets::heading(tree, root, "Keybindings");
        let header = widgets::caption_row(tree, root, "");
        for column in COLUMNS {
            let label = widgets::note(tree, header, column);
            widgets::set_width(tree, label, Sizing::Px(CELL));
        }
        let mut style = LayoutStyle::column().with_gap(theme::GAP);
        style.max_height = Some(200.0);
        let list = tree.scroll_panel(root, style);
        let rows = BINDABLE_ACTIONS
            .iter()
            .map(|(_, label)| {
                let row = widgets::caption_row(tree, list, label);
                let [a, b, c] =
                    COLUMNS.map(|column| widgets::button(tree, row, &format!("{label} {column}")));
                for button in [a, b, c] {
                    widgets::set_width(tree, button, Sizing::Px(CELL));
                }
                let reset = widgets::button(tree, row, &format!("Reset {label}"));
                tree.set_text(reset, "Reset");
                [a, b, c, reset]
            })
            .collect::<Vec<_>>();
        let actions = widgets::row(tree, root);
        let all = widgets::button(tree, actions, "Reset all");
        let back = widgets::button(tree, actions, "Back");
        tree.focus(rows[0][0]);
        theme::slide_in(tree, root);
        stack(ctx.world).own_root(root, RootCover::Hide);
        self.ui = Some((root, rows, all, back));
        self.refresh(ctx.world);
    }

    fn on_exit(&mut self, ctx: &mut StateContext) {
        self.stop_capture(ctx.world);
    }

    fn on_resume(&mut self, ctx: &mut StateContext) {
        self.refresh(ctx.world);
    }

    fn update(&mut self, world: &mut World) {
        let Some((root, rows, all, back)) = self.ui.clone() else {
            return;
        };
        if transitioning(world) || flow_pending(world) {
            return;
        }
        if let Some((row, column, left)) = self.capture.as_mut() {
            *left -= world.resource::<Time>().real_delta();
            let (row, column, left) = (*row, *column, *left);
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
            if left <= 0.0 {
                return self.stop_capture(world);
            }
            let wanted = if column == 2 { "button" } else { "key" };
            let text = format!("Press a {wanted}… ({} s)", left.ceil());
            let tree = world.resource_mut::<UiTree>();
            tree.set_text(rows[row][column], text.as_str());
            return;
        }
        let buttons: Vec<WidgetId> = rows.iter().flatten().copied().chain([all, back]).collect();
        match clicked(world, &buttons) {
            Some(at) if buttons[at] == back => pop(world),
            Some(at) if buttons[at] == all => {
                reset(world, None);
                self.refresh(world);
            }
            Some(at) if at % 4 == 3 => {
                reset(world, Some(BINDABLE_ACTIONS[at / 4].0));
                self.refresh(world);
            }
            Some(at) => {
                self.capture = Some((at / 4, at % 4, CAPTURE_SECS));
                let router = world.resource_mut::<UiRouter>();
                router.capture_next_input(buttons[at]);
            }
            None if cancelled(world, root) => pop(world),
            None => {}
        }
    }
}
