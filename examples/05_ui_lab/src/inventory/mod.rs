//! An inventory screen: a slot grid, a detail pane that follows focus and
//! a drop confirmation, driven by every device. The mouse clicks slots and
//! buttons; keys and the pad move focus between slots and onto the buttons,
//! accept with `ui_accept` and close with `ui_cancel`. Three actions act on
//! the focused slot from anywhere on the screen: `inventory_use` (U, pad
//! West), `inventory_equip` (E, pad North) and `inventory_drop` (Delete, pad
//! Select), added in code ([`add_bindings`]) and shown in a prompt row for
//! the last device.
//!
//! The pattern for a modal screen's hotkeys: a shown modal root takes every
//! key and pad press from the routed view (`D-149`), so Escape and East
//! always reach `ui_cancel`, never `engine_exit`. The hotkeys therefore
//! read the raw `InputState` through `ActionMap::just_pressed`, and act only
//! while the screen is live: its root shown and not frozen (an owned root
//! freezes while its state is covered or a transition runs, `D-163`), the
//! confirmation hidden, no transition running and no capture pending.
//!
//! [`InventoryState`] is the screen as a state: play pushes it on
//! `inventory_open` (I, pad North), as does the pause menu's "Inventory",
//! and it owns its roots with `Hide`. The paint-path fixtures build the same
//! window directly ([`build`]) with the confirmation open, and drive it with
//! [`inventory_system`].

pub mod detail;
pub mod grid;
pub mod items;

use tungsten::core::ui::{Align, Anchor, LayoutStyle, Sizing, UiLayer};
use tungsten::core::ui::{UiEvent, UiRouter, UiTree, Visibility, WidgetId};
use tungsten::core::{
    ActionMap, AssetRegistry, Binding, EventQueue, GamepadButton, InputState, KeyCode, World,
};
use tungsten::{GameState, RootCover, StateContext, StateId, StateStack};

use glam::Vec2;

use self::detail::{Confirm, Pane, Shown};
use self::grid::Grid;
use self::items::{ITEMS, ItemKind, SLOTS, START};
use crate::theme;
use crate::widgets;

/// The window frame's sprite ID, a nine-slice.
pub const FRAME_ID: &str = "ex05_frame";

/// Uses one of the focused consumable: U, pad West.
pub const USE_ACTION: &str = "inventory_use";
/// Equips or unequips the focused gear: E, pad North.
pub const EQUIP_ACTION: &str = "inventory_equip";
/// Asks to drop the focused stack: Delete, pad Select.
pub const DROP_ACTION: &str = "inventory_drop";
/// Opens the inventory from play: I, pad North.
pub const OPEN_ACTION: &str = "inventory_open";

/// Adds the inventory's actions to the action map, once each, as
/// `add_pause_pad_binding` adds Start.
pub fn add_bindings(world: &mut World) {
    let Some(map) = world.get_resource_mut::<ActionMap>() else {
        return;
    };
    for (action, key, button) in [
        (USE_ACTION, KeyCode::KeyU, GamepadButton::West),
        (EQUIP_ACTION, KeyCode::KeyE, GamepadButton::North),
        (DROP_ACTION, KeyCode::Delete, GamepadButton::Select),
        (OPEN_ACTION, KeyCode::KeyI, GamepadButton::North),
    ] {
        let mut bindings = map.bindings(action).to_vec();
        for binding in [
            Binding::Key { code: key },
            Binding::GamepadButton { button },
        ] {
            if !bindings.contains(&binding) {
                bindings.push(binding);
            }
        }
        map.replace_bindings(action, bindings);
    }
}

/// A slot's stack: an item index into [`ITEMS`] and its count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stack {
    pub item: usize,
    pub count: u32,
}

/// The screen's widgets.
#[derive(Debug, Clone)]
pub struct InventoryUi {
    /// The modal window root.
    pub root: WidgetId,
    pub grid: Grid,
    pub pane: Pane,
    pub confirm: Confirm,
    /// The prompt row naming the actions' bindings.
    pub prompt: WidgetId,
    /// Close: leaves the screen, as `ui_cancel` does.
    pub close: WidgetId,
}

/// The inventory: its widgets and contents, a `World` resource while the
/// screen exists.
#[derive(Debug)]
pub struct Inventory {
    pub ui: InventoryUi,
    /// The slots' stacks.
    pub contents: [Option<Stack>; SLOTS],
    /// The slot equipped per gear kind: weapon, armour, trinket.
    pub equipped: [Option<usize>; 3],
    /// The slot the pane shows: the focused one, or the last focused.
    pub selected: usize,
    /// The slot the confirmation asks to drop.
    pending_drop: Option<usize>,
    /// What the pane shows now.
    shown: Option<(usize, Shown)>,
    /// What the slots show now: a slot changes only when its stack does.
    slots_shown: [Option<Stack>; SLOTS],
}

/// The prompt row's template.
const PROMPT: &str = "{action:inventory_use} use   {action:inventory_equip} equip   \
                      {action:inventory_drop} drop   {action:ui_cancel} close";

/// Builds the inventory into `tree`: the window, a modal root on the popup
/// layer centred in the viewport, holding the nine-slice frame, a heading,
/// the grid beside the pane and the prompt row with Close; and the hidden
/// confirmation. Focus starts on the first slot, or on the confirmation's
/// Cancel when `confirm_open`, which asks to drop the first slot's stack.
pub fn build(tree: &mut UiTree, sprites: &mut AssetRegistry, confirm_open: bool) -> Inventory {
    let mut contents = [None; SLOTS];
    for (slot, item, count) in START {
        contents[slot] = Some(Stack { item, count });
    }
    let frame = sprites.intern_sprite(FRAME_ID);
    let width =
        2.0 * theme::WINDOW_PADDING + grid::size().x + theme::SECTION_GAP + theme::PANE_WIDTH;
    let mut style = LayoutStyle::column()
        .with_padding(theme::WINDOW_PADDING)
        .with_gap(theme::SECTION_GAP)
        .with_size(Sizing::Px(width), Sizing::Auto)
        .anchored(Anchor::Center, Vec2::ZERO);
    style.align_items = Align::Stretch;
    let root = tree.add_root(UiLayer::GamePopup, style);
    tree.set_modal(root, true);
    let frame = tree.image(
        root,
        frame,
        Vec2::new(48.0, 48.0),
        LayoutStyle::default()
            .with_size(Sizing::Fill, Sizing::Fill)
            .anchored(Anchor::TopStart, Vec2::ZERO),
    );
    tree.set_hit_testable(frame, false);
    widgets::heading(tree, root, "Inventory");
    let mut body_style = LayoutStyle::row().with_gap(theme::SECTION_GAP);
    body_style.align_items = Align::Start;
    let body = tree.panel(root, body_style);
    tree.set_overrides(body, widgets::transparent());
    let grid = grid::build(tree, body, sprites, &contents);
    let pane = detail::build(tree, body);
    let mut footer_style = LayoutStyle::row().with_gap(theme::ROW_GAP);
    footer_style.align_items = Align::Center;
    let footer = tree.panel(root, footer_style);
    tree.set_overrides(footer, widgets::transparent());
    let prompt = widgets::prompt_row(tree, footer, PROMPT);
    tree.set_layout_style(
        prompt,
        LayoutStyle::default().with_size(Sizing::Fill, Sizing::Auto),
    );
    let close = widgets::button(tree, footer, "Close");
    let confirm = detail::build_confirm(tree);
    let mut inventory = Inventory {
        ui: InventoryUi {
            root,
            grid,
            pane,
            confirm,
            prompt,
            close,
        },
        contents,
        equipped: [None; 3],
        selected: 0,
        pending_drop: None,
        shown: None,
        slots_shown: contents,
    };
    tree.focus(inventory.ui.grid.slots[0].root);
    if confirm_open {
        inventory.open_confirm(tree, false);
    }
    inventory.refresh(tree);
    inventory
}

impl Inventory {
    /// The slot index of `widget`, a slot's root.
    fn slot_of(&self, widget: WidgetId) -> Option<usize> {
        self.ui
            .grid
            .slots
            .iter()
            .position(|slot| slot.root == widget)
    }

    /// Whether the confirmation shows.
    #[must_use]
    pub fn confirming(&self, tree: &UiTree) -> bool {
        tree.visibility(self.ui.confirm.root) == Some(Visibility::Visible)
    }

    /// Uses one of the selected consumable; its last empties the slot.
    fn use_selected(&mut self) {
        let slot = self.selected;
        if let Some(stack) = &mut self.contents[slot]
            && ITEMS[stack.item].kind == ItemKind::Consumable
        {
            stack.count -= 1;
            if stack.count == 0 {
                self.contents[slot] = None;
            }
        }
    }

    /// Equips the selected gear, unequipping its kind's other item, or
    /// unequips it.
    fn toggle_equip(&mut self) {
        let slot = self.selected;
        let Some(gear) = self.contents[slot].and_then(|stack| ITEMS[stack.item].kind.gear_slot())
        else {
            return;
        };
        self.equipped[gear] = if self.equipped[gear] == Some(slot) {
            None
        } else {
            Some(slot)
        };
    }

    /// Shows the confirmation for the selected stack, Cancel focused,
    /// fading it in when `pop`.
    fn open_confirm(&mut self, tree: &mut UiTree, pop: bool) {
        let Some(stack) = self.contents[self.selected] else {
            return;
        };
        self.pending_drop = Some(self.selected);
        let confirm = self.ui.confirm;
        tree.set_text(confirm.heading, format!("Drop {}?", ITEMS[stack.item].name));
        tree.set_visibility(confirm.root, Visibility::Visible);
        if pop {
            theme::pop_in(tree, confirm.root);
        }
        tree.focus(confirm.cancel);
    }

    /// Hides the confirmation, dropping its stack when `drop`, and puts
    /// focus back on the slot.
    fn close_confirm(&mut self, tree: &mut UiTree, drop: bool) {
        tree.set_visibility(self.ui.confirm.root, Visibility::Hidden);
        let Some(slot) = self.pending_drop.take() else {
            return;
        };
        if drop {
            self.contents[slot] = None;
            for equipped in &mut self.equipped {
                if *equipped == Some(slot) {
                    *equipped = None;
                }
            }
        }
        tree.focus(self.ui.grid.slots[slot].root);
    }

    /// Selects the focused slot, when focus is on one.
    fn follow_focus(&mut self, tree: &UiTree) {
        if let Some(slot) = tree.focused().and_then(|focused| self.slot_of(focused)) {
            self.selected = slot;
        }
    }

    /// Follows focus onto a slot, then shows what changed: the selected
    /// slot in the pane and every slot's stack.
    fn refresh(&mut self, tree: &mut UiTree) {
        self.follow_focus(tree);
        for (slot, ui) in self.ui.grid.slots.iter().enumerate() {
            if self.slots_shown[slot] != self.contents[slot] {
                self.slots_shown[slot] = self.contents[slot];
                grid::show_slot(tree, ui, self.contents[slot]);
            }
        }
        let stack = self.contents[self.selected];
        let shown = Shown {
            item: stack.map(|stack| (&ITEMS[stack.item], stack.count)),
            equipped: self.equipped.contains(&Some(self.selected)),
        };
        if self.shown != Some((self.selected, shown)) {
            self.shown = Some((self.selected, shown));
            detail::show(tree, &self.ui.pane, shown);
        }
        if tree.focused().is_none() {
            tree.focus(self.ui.grid.slots[self.selected].root);
        }
    }
}

/// Whether the screen takes its hotkeys now: its root shown and not
/// frozen, the confirmation hidden, no transition running and no capture
/// pending.
fn live(world: &World, inventory: &Inventory) -> bool {
    let tree = world.resource::<UiTree>();
    tree.visibility(inventory.ui.root) == Some(Visibility::Visible)
        && !tree.frozen(inventory.ui.root)
        && !inventory.confirming(tree)
        && !world
            .get_resource::<StateStack>()
            .is_some_and(StateStack::is_transitioning)
        && world
            .get_resource::<UiRouter>()
            .is_none_or(|router| router.capturing().is_none())
}

/// Whether `action` was just pressed in the raw input.
fn hotkey(world: &World, action: &str) -> bool {
    world
        .get_resource::<ActionMap>()
        .zip(world.get_resource::<InputState>())
        .is_some_and(|(map, input)| map.just_pressed(input, action))
}

/// One frame of the inventory: this frame's events, then the hotkeys while
/// the screen is live, then the pane and the slots. Returns whether Close
/// or `ui_cancel` on the window asked to leave.
pub fn update(world: &mut World) -> bool {
    let Some(mut inventory) = world.remove_resource::<Inventory>() else {
        return false;
    };
    let events: Vec<UiEvent> = world
        .get_resource::<EventQueue<UiEvent>>()
        .map(|queue| queue.iter_current().copied().collect())
        .unwrap_or_default();
    let (pane, confirm, root, close) = (
        inventory.ui.pane,
        inventory.ui.confirm,
        inventory.ui.root,
        inventory.ui.close,
    );
    // This frame's routing may have moved focus: the actions act on it.
    inventory.follow_focus(world.resource::<UiTree>());
    let mut leave = false;
    for event in events {
        let tree = world.resource_mut::<UiTree>();
        match event {
            UiEvent::Activated { widget, .. } if widget == pane.use_button => {
                inventory.use_selected();
            }
            UiEvent::Activated { widget, .. } if widget == pane.equip => {
                inventory.toggle_equip();
            }
            UiEvent::Activated { widget, .. } if widget == pane.drop => {
                inventory.open_confirm(tree, true);
            }
            UiEvent::Activated { widget, .. } if widget == confirm.cancel => {
                inventory.close_confirm(tree, false);
            }
            UiEvent::Cancel { scope, .. } if scope == confirm.root => {
                inventory.close_confirm(tree, false);
            }
            UiEvent::Activated { widget, .. } if widget == confirm.drop => {
                inventory.close_confirm(tree, true);
            }
            UiEvent::Activated { widget, .. } if widget == close => leave = true,
            UiEvent::Cancel { scope, .. } if scope == root => leave = true,
            _ => {}
        }
    }
    if live(world, &inventory) {
        if hotkey(world, USE_ACTION) {
            inventory.use_selected();
        }
        if hotkey(world, EQUIP_ACTION) {
            inventory.toggle_equip();
        }
        if hotkey(world, DROP_ACTION) {
            inventory.open_confirm(world.resource_mut::<UiTree>(), true);
        }
    }
    inventory.refresh(world.resource_mut::<UiTree>());
    world.insert_resource(inventory);
    leave
}

/// Drives an inventory the paint-path fixtures built directly, with no
/// state around it; Close and `ui_cancel` do nothing there.
pub fn inventory_system(world: &mut World) {
    let _ = update(world);
}

/// The inventory as a state: built on enter, its roots owned with `Hide`,
/// sliding in; Close or `ui_cancel` pops it behind a fade.
pub struct InventoryState;

impl GameState for InventoryState {
    fn id(&self) -> StateId {
        "inventory"
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        let Some(mut tree) = ctx.world.remove_resource::<UiTree>() else {
            return;
        };
        let inventory = {
            let sprites = ctx.world.resource_mut::<AssetRegistry>();
            build(&mut tree, sprites, false)
        };
        theme::slide_in(&mut tree, inventory.ui.root);
        ctx.world.insert_resource(tree);
        let stack = ctx.world.resource_mut::<StateStack>();
        stack.own_root(inventory.ui.root, RootCover::Hide);
        stack.own_root(inventory.ui.confirm.root, RootCover::Hide);
        ctx.world.insert_resource(inventory);
    }

    fn on_exit(&mut self, ctx: &mut StateContext) {
        ctx.world.remove_resource::<Inventory>();
    }

    fn update(&mut self, world: &mut World) {
        let leave = update(world);
        if leave && !world.resource::<StateStack>().is_transitioning() {
            world
                .resource_mut::<StateStack>()
                .request_pop_transition(theme::fade());
        }
    }
}
