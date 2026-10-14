//! UI input routing (`D-149`): the engine-owned `ui_*` action names, the
//! [`UiRouter`] resource, its timing settings, the last device used, and
//! [`ui_route_system`], which once a frame turns the ordered raw input into
//! hover, press, capture, focus and [`UiEvent`]s against the last committed
//! layout and feeds [`RoutedInput`] what the UI did not take.

use glam::Vec2;

use crate::display::ScaleFactor;
use crate::ecs::{EventQueue, World};
use crate::input::{
    ActionMap, Binding, InputState, KeyCode, MouseButton, RawEvent, RawInput, RoutedInput,
    WheelDelta,
};
use crate::time::Time;

use super::event::UiEvent;
use super::focus::NavDirection;
use super::id::WidgetId;
use super::node::CursorIcon;
use super::paint::UiDebug;
use super::tree::UiTree;

/// Activates the focused widget; Enter and Space by default.
pub const UI_ACCEPT: &str = "ui_accept";
/// Backs out of the focused scope or the shown modal root; Escape by
/// default.
pub const UI_CANCEL: &str = "ui_cancel";
/// Moves focus to the next widget in tab order, or back with Shift held;
/// Tab by default.
pub const UI_NEXT: &str = "ui_next";
/// Moves focus back in tab order; unbound by default, since Shift with
/// [`UI_NEXT`] does it and bindings carry no modifiers (`D-045`).
pub const UI_PREV: &str = "ui_prev";
/// Moves focus up ([`NavDirection::Up`]); the up arrow by default.
pub const UI_UP: &str = "ui_up";
/// Moves focus down; the down arrow by default.
pub const UI_DOWN: &str = "ui_down";
/// Moves focus left; the left arrow by default.
pub const UI_LEFT: &str = "ui_left";
/// Moves focus right; the right arrow by default.
pub const UI_RIGHT: &str = "ui_right";

const ACCEPT: u8 = 1;
const CANCEL: u8 = 1 << 1;
const NEXT: u8 = 1 << 2;
const PREV: u8 = 1 << 3;
const DIRECTIONS: [(u8, NavDirection); 4] = [
    (1 << 4, NavDirection::Up),
    (1 << 5, NavDirection::Down),
    (1 << 6, NavDirection::Left),
    (1 << 7, NavDirection::Right),
];
const ACTIONS: [(&str, u8); 8] = [
    (UI_ACCEPT, ACCEPT),
    (UI_CANCEL, CANCEL),
    (UI_NEXT, NEXT),
    (UI_PREV, PREV),
    (UI_UP, DIRECTIONS[0].0),
    (UI_DOWN, DIRECTIONS[1].0),
    (UI_LEFT, DIRECTIONS[2].0),
    (UI_RIGHT, DIRECTIONS[3].0),
];

/// The device that last drove the UI (`D-149`): focus shows after the
/// keyboard and hides after the pointer.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum InputDevice {
    /// A key press. The device at start, so a focus set at build shows.
    #[default]
    Keyboard,
    /// A mouse button press or the wheel; a move alone changes nothing.
    Pointer,
}

/// Navigation repeat and double-click timing (`D-149`), on the real clock.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiRouteSettings {
    /// Seconds a held direction waits before it repeats; 0.5.
    pub repeat_delay: f32,
    /// Seconds between the repeats after that; 0.1. A routing moves focus
    /// at most once.
    pub repeat_interval: f32,
    /// The most seconds between two activations of one widget that make a
    /// double click; 0.5.
    pub double_click_time: f32,
    /// The farthest the pointer may move between them, in UI units; 4.
    pub double_click_distance: f32,
}

impl Default for UiRouteSettings {
    fn default() -> Self {
        Self {
            repeat_delay: 0.5,
            repeat_interval: 0.1,
            double_click_time: 0.5,
            double_click_distance: 4.0,
        }
    }
}

/// A held direction and when it next moves focus.
#[derive(Debug, Clone, Copy)]
struct Repeat {
    dir: NavDirection,
    key: KeyCode,
    /// Router clock seconds.
    next: f64,
}

/// The routing state: a `World` resource that [`ui_route_system`] updates
/// once a frame, in `pre_update`, and the game reads (`D-149`).
#[derive(Debug, Default)]
pub struct UiRouter {
    settings: UiRouteSettings,
    last_device: InputDevice,
    pointer_over_ui: bool,
    wants_keyboard: bool,
    cursor: CursorIcon,
    /// Real seconds summed over the routings.
    clock: f64,
    /// The pointer in physical pixels; `None` before the first move, after
    /// it leaves the window and after focus loss.
    pointer: Option<Vec2>,
    /// The hovered target, as the last events reported it.
    hovered: Option<WidgetId>,
    /// The widget a primary press captured, until its release.
    captured: Option<WidgetId>,
    /// Other buttons' presses on targets, until their releases.
    presses: Vec<(MouseButton, WidgetId)>,
    /// Keys and buttons whose presses the UI took: their releases are its.
    ui_keys: Vec<KeyCode>,
    ui_buttons: Vec<MouseButton>,
    repeat: Option<Repeat>,
    /// The last click activation: widget, router clock, position in UI
    /// units.
    last_click: Option<(WidgetId, f64, Vec2)>,
    /// The focus the last events reported.
    reported_focus: Option<WidgetId>,
    /// This routing's stream, events and routed input, reused.
    stream: Vec<RawEvent>,
    events: Vec<UiEvent>,
    routed: Vec<RawInput>,
    /// The `ui_*` actions of each key pressed in this routing's stream.
    key_actions: Vec<(KeyCode, u8)>,
}

impl UiRouter {
    /// The timing settings.
    #[must_use]
    pub fn settings(&self) -> &UiRouteSettings {
        &self.settings
    }

    /// The timing settings, to change; the next routing reads them.
    pub fn settings_mut(&mut self) -> &mut UiRouteSettings {
        &mut self.settings
    }

    /// The device of the last key, button or wheel input routed; `Keyboard`
    /// before any.
    #[must_use]
    pub fn last_device(&self) -> InputDevice {
        self.last_device
    }

    /// Whether, at the last routing, the pointer was over a widget of a root
    /// that takes input, or anywhere in the window while a modal root was
    /// shown: a press or the wheel there is the UI's.
    #[must_use]
    pub fn pointer_over_ui(&self) -> bool {
        self.pointer_over_ui
    }

    /// Whether the UI had the keyboard at the last routing: while a modal
    /// root is shown, every key press is the UI's; while a widget holds
    /// visible focus, the presses of keys bound to the `ui_*` actions are.
    #[must_use]
    pub fn wants_keyboard(&self) -> bool {
        self.wants_keyboard
    }

    /// The system cursor the window shows: the nearest one set
    /// ([`UiTree::set_cursor`]) on the captured widget or its ancestors,
    /// else on the widget under the pointer or its ancestors, else
    /// [`CursorIcon::Default`].
    #[must_use]
    pub fn cursor(&self) -> CursorIcon {
        self.cursor
    }

    /// One routing: reads the resources, routes the stream against the
    /// tree, then feeds the routed view and the event queue.
    fn route(&mut self, world: &mut World) {
        self.clock += f64::from(world.get_resource::<Time>().map_or(0.0, Time::real_delta));
        let scale = world
            .get_resource::<ScaleFactor>()
            .map_or(1.0, ScaleFactor::get);
        let hide = world
            .get_resource::<UiDebug>()
            .is_some_and(|debug| debug.hide_game_layers);
        let mut stream = std::mem::take(&mut self.stream);
        stream.clear();
        if let Some(input) = world.get_resource::<InputState>() {
            stream.extend_from_slice(input.raw_events());
        }
        let mut key_actions = std::mem::take(&mut self.key_actions);
        key_actions.clear();
        let presses = stream.iter().filter_map(|event| match event.input {
            RawInput::Key {
                code,
                pressed: true,
                repeat: false,
                ..
            } => Some(code),
            _ => None,
        });
        if let Some(map) = world.get_resource::<ActionMap>() {
            for code in presses {
                if !key_actions.iter().any(|(key, _)| *key == code) {
                    key_actions.push((code, ui_actions(map, code)));
                }
            }
        }
        self.events.clear();
        self.routed.clear();
        let mut fallback = None;
        let tree = match world.get_resource_mut::<UiTree>() {
            Some(tree) => tree,
            None => fallback.insert(UiTree::new()),
        };
        Routing {
            modal: None,
            router: self,
            tree,
            scale,
            hide,
            key_actions: &key_actions,
        }
        .run(&stream);
        if let Some(routed) = world.get_resource_mut::<RoutedInput>() {
            routed.begin_frame();
            for input in &self.routed {
                feed(routed, *input);
            }
        }
        if !self.events.is_empty()
            && world.has_event::<UiEvent>()
            && let Some(queue) = world.get_resource_mut::<EventQueue<UiEvent>>()
        {
            for event in &self.events {
                queue.send(*event);
            }
        }
        self.stream = stream;
        self.key_actions = key_actions;
    }
}

/// Routes the frame's input through the UI (`D-149`), once a frame in
/// `pre_update` after the state dispatcher; the umbrella's `UiPlugin`
/// registers it as `ui_route`. Reads [`InputState::raw_events`] in arrival
/// order against the [`UiTree`]'s last committed layout, pointer positions
/// divided by [`ScaleFactor::get`], timers on [`Time::real_delta`];
/// updates the tree's hover, press and focus and the [`UiRouter`]; sends
/// [`UiEvent`]s when `EventQueue<UiEvent>` is registered; and feeds
/// [`RoutedInput`], after its `begin_frame`, the input the UI did not take.
/// A world without a `UiRouter` gets one; without a `UiTree`, every input
/// is the game's.
pub fn ui_route_system(world: &mut World) {
    let mut router = world
        .get_resource_mut::<UiRouter>()
        .map(std::mem::take)
        .unwrap_or_default();
    router.route(world);
    match world.get_resource_mut::<UiRouter>() {
        Some(slot) => *slot = router,
        None => world.insert_resource(router),
    }
}

/// The `ui_*` actions `code` is bound to, as bits.
fn ui_actions(map: &ActionMap, code: KeyCode) -> u8 {
    ACTIONS
        .iter()
        .filter(|(action, _)| map.bindings(action).contains(&Binding::Key { code }))
        .fold(0, |bits, (_, bit)| bits | bit)
}

/// Applies one input to the routed view through `InputState`'s mutators. An
/// OS repeat reaches it only for a key it holds.
fn feed(routed: &mut InputState, input: RawInput) {
    match input {
        RawInput::Key {
            code,
            pressed: true,
            repeat,
            ..
        } => {
            if !repeat || routed.is_pressed(code) {
                routed.key_down(code);
            }
        }
        RawInput::Key {
            code,
            pressed: false,
            ..
        } => routed.key_up(code),
        RawInput::MouseButton {
            button,
            pressed: true,
        } => routed.mouse_down(button),
        RawInput::MouseButton {
            button,
            pressed: false,
        } => routed.mouse_up(button),
        RawInput::PointerMoved { x, y } => routed.update_cursor_position(x, y),
        RawInput::PointerLeft => routed.pointer_left(),
        RawInput::Wheel {
            delta: WheelDelta::Lines(x, y),
        } => routed.add_scroll_line_delta(x, y),
        RawInput::Wheel {
            delta: WheelDelta::Pixels(x, y),
        } => routed.add_scroll_pixel_delta(x, y),
        RawInput::Modifiers(modifiers) => routed.set_modifiers(modifiers),
        RawInput::Focus(focused) => routed.set_focused(focused),
    }
}

/// One routing's borrowed state.
struct Routing<'a> {
    router: &'a mut UiRouter,
    tree: &'a mut UiTree,
    scale: f32,
    hide: bool,
    key_actions: &'a [(KeyCode, u8)],
    /// The top modal root that takes input; fixed for the routing, since
    /// only game code shows or hides roots.
    modal: Option<WidgetId>,
}

impl Routing<'_> {
    fn run(&mut self, stream: &[RawEvent]) {
        self.modal = self
            .tree
            .roots()
            .iter()
            .rev()
            .copied()
            .find(|root| self.tree.root_routes(*root, self.hide) && self.tree.modal(*root));
        self.tree
            .set_focus_visible(self.router.last_device == InputDevice::Keyboard);
        self.sync_focus();
        self.check_presses();
        self.focus_into_modal();
        self.update_hover();
        for event in stream {
            self.event(event.input);
        }
        self.tick_repeat();
        self.sync_focus();
        self.finish();
    }

    fn event(&mut self, input: RawInput) {
        match input {
            RawInput::PointerMoved { x, y } => {
                self.router.pointer = Some(Vec2::new(x, y));
                self.update_hover();
                self.router.routed.push(input);
            }
            RawInput::PointerLeft => {
                self.cancel_presses();
                self.router.pointer = None;
                self.update_hover();
                self.router.routed.push(input);
            }
            RawInput::MouseButton {
                button,
                pressed: true,
            } => self.button_down(button),
            RawInput::MouseButton {
                button,
                pressed: false,
            } => self.button_up(button),
            RawInput::Wheel { .. } => {
                self.set_device(InputDevice::Pointer);
                if !self.pointer_takes() {
                    self.router.routed.push(input);
                }
            }
            RawInput::Key {
                code, repeat: true, ..
            } => {
                if !self.router.ui_keys.contains(&code) {
                    self.router.routed.push(input);
                }
            }
            RawInput::Key {
                code,
                pressed: true,
                modifiers,
                ..
            } => self.key_down(code, modifiers.shift(), input),
            RawInput::Key {
                code,
                pressed: false,
                ..
            } => self.key_up(code, input),
            RawInput::Modifiers(_) => self.router.routed.push(input),
            RawInput::Focus(focused) => {
                if !focused {
                    self.focus_lost();
                }
                self.router.routed.push(input);
            }
        }
    }

    // ---- pointer ----

    /// The pointer in UI units.
    fn point(&self) -> Option<Vec2> {
        self.router.pointer.map(|pointer| pointer / self.scale)
    }

    /// The widget under the pointer, and its target: the nearest
    /// interactive ancestor-or-self that is enabled, its ancestors too.
    fn hit(&self) -> (Option<WidgetId>, Option<WidgetId>) {
        let Some(point) = self.point() else {
            return (None, None);
        };
        let hit = self.tree.hit_test_layers(point, self.hide);
        let mut target = hit;
        while let Some(id) = target {
            if self.tree.interactive(id) && self.tree.is_enabled_in_tree(id) {
                break;
            }
            target = self.tree.parent(id);
        }
        (hit, target)
    }

    /// Whether a press or the wheel now is the UI's.
    fn pointer_takes(&self) -> bool {
        self.modal.is_some() || self.hit().0.is_some()
    }

    /// Hovers the target under the pointer, or only the captured widget
    /// while there is one, and shows that one pressed while it hovers.
    fn update_hover(&mut self) {
        let target = self.hit().1;
        let hovered = match self.router.captured {
            Some(captured) => target.filter(|target| *target == captured),
            None => target,
        };
        if hovered != self.router.hovered {
            if let Some(widget) = self.router.hovered {
                self.router.events.push(UiEvent::PointerLeft { widget });
            }
            if let Some(widget) = hovered {
                self.router.events.push(UiEvent::PointerEntered { widget });
            }
            self.router.hovered = hovered;
        }
        self.tree.set_hovered(hovered);
        self.tree.set_pressed(
            self.router
                .captured
                .filter(|captured| hovered == Some(*captured)),
        );
    }

    fn button_down(&mut self, button: MouseButton) {
        self.set_device(InputDevice::Pointer);
        if !self.pointer_takes() {
            self.router.routed.push(RawInput::MouseButton {
                button,
                pressed: true,
            });
            return;
        }
        self.router.ui_buttons.push(button);
        let Some(widget) = self.hit().1 else {
            return;
        };
        self.router.events.push(UiEvent::Pressed { widget, button });
        if button == MouseButton::Left {
            self.router.captured = Some(widget);
            self.tree.focus(widget);
            self.sync_focus();
            self.update_hover();
        } else {
            self.router.presses.push((button, widget));
        }
    }

    fn button_up(&mut self, button: MouseButton) {
        let Some(at) = self.router.ui_buttons.iter().position(|b| *b == button) else {
            self.router.routed.push(RawInput::MouseButton {
                button,
                pressed: false,
            });
            return;
        };
        self.router.ui_buttons.swap_remove(at);
        if button == MouseButton::Left {
            if let Some(widget) = self.router.captured.take() {
                self.router
                    .events
                    .push(UiEvent::Released { widget, button });
                if self.router.hovered == Some(widget) {
                    self.router.events.push(UiEvent::Activated { widget });
                    self.double_click(widget);
                }
                self.update_hover();
            }
        } else if let Some(at) = self.router.presses.iter().position(|(b, _)| *b == button) {
            let (_, widget) = self.router.presses.remove(at);
            self.router
                .events
                .push(UiEvent::Released { widget, button });
        }
    }

    /// After a click activated `widget`: a double click when the last one
    /// activated it too, soon enough and near enough.
    fn double_click(&mut self, widget: WidgetId) {
        let at = self.point().unwrap_or_default();
        let settings = self.router.settings;
        let clock = self.router.clock;
        let double = self.router.last_click.is_some_and(|(last, time, from)| {
            last == widget
                && clock - time <= f64::from(settings.double_click_time)
                && from.distance(at) <= settings.double_click_distance
        });
        if double {
            self.router.events.push(UiEvent::DoubleClicked { widget });
            self.router.last_click = None;
        } else {
            self.router.last_click = Some((widget, clock, at));
        }
    }

    /// Ends every press without a release.
    fn cancel_presses(&mut self) {
        if let Some(widget) = self.router.captured.take() {
            self.router.events.push(UiEvent::PressCancelled { widget });
        }
        for &(_, widget) in &self.router.presses {
            self.router.events.push(UiEvent::PressCancelled { widget });
        }
        self.router.presses.clear();
        self.tree.set_pressed(None);
    }

    /// Ends the presses whose widget was removed, hidden or disabled, or
    /// whose root stopped taking input, since the last routing.
    fn check_presses(&mut self) {
        if let Some(widget) = self.router.captured
            && !self.takes_input(widget)
        {
            self.router.captured = None;
            self.router.events.push(UiEvent::PressCancelled { widget });
            self.tree.set_pressed(None);
        }
        let mut i = 0;
        while i < self.router.presses.len() {
            let (_, widget) = self.router.presses[i];
            if self.takes_input(widget) {
                i += 1;
            } else {
                self.router.presses.remove(i);
                self.router.events.push(UiEvent::PressCancelled { widget });
            }
        }
    }

    fn takes_input(&self, widget: WidgetId) -> bool {
        self.tree.is_shown(widget)
            && self.tree.is_enabled_in_tree(widget)
            && self
                .tree
                .root_of(widget)
                .is_some_and(|root| self.tree.root_routes(root, self.hide))
    }

    // ---- keyboard ----

    /// The focused widget when keys act on it: in a root that takes input,
    /// and not under the shown modal root.
    fn focus_in_play(&self) -> Option<WidgetId> {
        let focused = self.tree.focused()?;
        let root = self.tree.root_of(focused)?;
        if !self.tree.root_routes(root, self.hide) {
            return None;
        }
        if let Some(modal) = self.modal {
            let roots = self.tree.roots();
            let at = |id: WidgetId| roots.iter().position(|root| *root == id);
            if matches!((at(root), at(modal)), (Some(a), Some(b)) if a < b) {
                return None;
            }
        }
        Some(focused)
    }

    /// A shown modal root with no focus in it or above it focuses its first
    /// focusable widget.
    fn focus_into_modal(&mut self) {
        if let Some(modal) = self.modal
            && self.focus_in_play().is_none()
            && let Some(first) = self.tree.first_focusable(modal)
        {
            self.tree.focus(first);
            self.sync_focus();
        }
    }

    fn key_down(&mut self, code: KeyCode, shift: bool, input: RawInput) {
        let actions = self
            .key_actions
            .iter()
            .find(|(key, _)| *key == code)
            .map_or(0, |(_, actions)| *actions);
        let takes = self.modal.is_some()
            || (actions != 0 && self.focus_in_play().is_some() && self.tree.focus_visible());
        self.set_device(InputDevice::Keyboard);
        if !takes {
            self.router.routed.push(input);
            return;
        }
        self.router.ui_keys.push(code);
        if actions & (NEXT | PREV) != 0 && self.focus_in_play().is_some() {
            if actions & PREV != 0 || shift {
                self.tree.focus_prev();
            } else {
                self.tree.focus_next();
            }
            self.sync_focus();
        }
        if let Some(&(_, dir)) = DIRECTIONS.iter().find(|(bit, _)| actions & bit != 0) {
            if self.focus_in_play().is_some() {
                self.tree.focus_direction(dir);
                self.sync_focus();
            }
            self.router.repeat = Some(Repeat {
                dir,
                key: code,
                next: self.router.clock + f64::from(self.router.settings.repeat_delay),
            });
        }
        if actions & ACCEPT != 0
            && let Some(widget) = self.focus_in_play()
        {
            self.router.events.push(UiEvent::Activated { widget });
        }
        if actions & CANCEL != 0 {
            let scope = self
                .focus_in_play()
                .and_then(|focused| self.tree.scope_of(focused))
                .or(self.modal);
            if let Some(scope) = scope {
                self.router.events.push(UiEvent::Cancel { scope });
            }
            if self.modal.is_none() {
                self.tree.blur();
                self.sync_focus();
            }
        }
    }

    fn key_up(&mut self, code: KeyCode, input: RawInput) {
        if self.router.repeat.is_some_and(|repeat| repeat.key == code) {
            self.router.repeat = None;
        }
        match self.router.ui_keys.iter().position(|key| *key == code) {
            Some(at) => {
                self.router.ui_keys.swap_remove(at);
            }
            None => self.router.routed.push(input),
        }
    }

    /// Moves focus once more for a held direction that is due, at most once
    /// a routing; a routing late by more than an interval does not catch up.
    fn tick_repeat(&mut self) {
        let Some(mut repeat) = self.router.repeat else {
            return;
        };
        let clock = self.router.clock;
        if clock < repeat.next {
            return;
        }
        if self.focus_in_play().is_some() {
            self.tree.focus_direction(repeat.dir);
            self.sync_focus();
        }
        let interval = f64::from(self.router.settings.repeat_interval);
        repeat.next += interval;
        if repeat.next <= clock {
            repeat.next = clock + interval;
        }
        self.router.repeat = Some(repeat);
    }

    /// Window focus loss: ends the presses, forgets the UI's held keys and
    /// buttons and the pointer, and clears the hover.
    fn focus_lost(&mut self) {
        self.cancel_presses();
        self.router.ui_keys.clear();
        self.router.ui_buttons.clear();
        self.router.repeat = None;
        self.router.pointer = None;
        self.update_hover();
    }

    // ---- shared ----

    fn set_device(&mut self, device: InputDevice) {
        self.router.last_device = device;
        self.tree.set_focus_visible(device == InputDevice::Keyboard);
    }

    /// Reports a focus change since the last report: the router's or the
    /// game's.
    fn sync_focus(&mut self) {
        let focused = self.tree.focused();
        if focused == self.router.reported_focus {
            return;
        }
        if let Some(widget) = self.router.reported_focus {
            self.router.events.push(UiEvent::FocusLost { widget });
        }
        if let Some(widget) = focused {
            self.router.events.push(UiEvent::FocusGained { widget });
        }
        self.router.reported_focus = focused;
    }

    /// The router's readouts after the stream.
    fn finish(&mut self) {
        let hit = self.hit().0;
        self.router.pointer_over_ui =
            self.router.pointer.is_some() && (hit.is_some() || self.modal.is_some());
        self.router.wants_keyboard =
            self.modal.is_some() || (self.focus_in_play().is_some() && self.tree.focus_visible());
        let mut source = self.router.captured.or(hit);
        let mut cursor = None;
        while let Some(id) = source {
            cursor = self.tree.cursor(id);
            if cursor.is_some() {
                break;
            }
            source = self.tree.parent(id);
        }
        self.router.cursor = cursor.unwrap_or_default();
    }
}

#[cfg(test)]
#[path = "../tests/ui/route.rs"]
mod tests;
