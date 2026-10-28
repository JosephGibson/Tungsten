//! UI input routing (`D-149`): the engine-owned `ui_*` action names, the
//! [`UiRouter`] resource, its timing settings, the last device used, and
//! [`ui_route_system`], which once a frame turns the ordered raw input into
//! hover, press, capture, focus and [`UiEvent`]s against the last committed
//! layout and feeds [`RoutedInput`] what the UI did not take. Gamepad
//! buttons and stick directions navigate as keys do (M54). An activation
//! flips a toggle; a primary press and the moves while it holds set a
//! slider from the pointer, and `ui_left` and `ui_right` step a focused one
//! (M55). A frozen root's widgets take no input, while the root still
//! blocks the pointer and, when modal, the keyboard (M56); the pointer maps
//! through translation and scale as paint does. M56 also adds the
//! capture-next-input mode for rebinding and resolves prompts after each
//! routing. M57's wheel moves the scroll panels under the pointer, and a
//! primary press on a drag handle drags its root once the pointer moves
//! past the drag threshold. M58 raises a `Role::Window` root on a primary
//! press over it, and drags a debug root and scrolls its panels in its own
//! units.

use glam::Vec2;

use crate::display::ScaleFactor;
use crate::ecs::{EventQueue, World};
use crate::input::{
    ActionMap, AxisDirection, Binding, GamepadAxis, GamepadButton, GamepadId, InputState, KeyCode,
    MouseButton, RawEvent, RawInput, RoutedInput, WheelDelta,
};
use crate::time::Time;

use super::control;
use super::event::{UiEvent, UiValue};
use super::focus::NavDirection;
use super::id::WidgetId;
use super::node::{CursorIcon, Role};
use super::paint::UiDebug;
use super::prompt;
use super::tree::UiTree;

/// Activates the focused widget; Enter and Space by default.
pub const UI_ACCEPT: &str = "ui_accept";
/// Backs out of the focused scope or the shown modal root; Escape by
/// default.
pub const UI_CANCEL: &str = "ui_cancel";
/// Moves focus to the next widget in tab order, or back with Shift held;
/// Tab by default.
pub const UI_NEXT: &str = "ui_next";
/// Moves focus back in tab order; the left shoulder by default, and on the
/// keyboard Shift with [`UI_NEXT`] does it, since bindings carry no
/// modifiers (`D-045`).
pub const UI_PREV: &str = "ui_prev";
/// Moves focus up ([`NavDirection::Up`]); the up arrow, the d-pad and the
/// left stick by default.
pub const UI_UP: &str = "ui_up";
/// Moves focus down; the down arrow, the d-pad and the left stick by default.
pub const UI_DOWN: &str = "ui_down";
/// Moves focus left; the left arrow, the d-pad and the left stick by default.
pub const UI_LEFT: &str = "ui_left";
/// Moves focus right; the right arrow, the d-pad and the left stick by
/// default.
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
/// keyboard or a gamepad and hides after the pointer.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum InputDevice {
    /// A key press. The device at start, so a focus set at build shows.
    #[default]
    Keyboard,
    /// A mouse button press or the wheel; a move alone changes nothing.
    Pointer,
    /// A gamepad button or stick direction press (M54).
    Gamepad,
}

/// A press that navigates, with its pad, so each pad's presses and releases
/// keep their owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NavInput {
    Key(KeyCode),
    Pad(GamepadId, GamepadButton),
    Stick(GamepadId, GamepadAxis, AxisDirection),
}

impl NavInput {
    /// The binding that names this press, pad aside.
    fn binding(self) -> Binding {
        match self {
            Self::Key(code) => Binding::Key { code },
            Self::Pad(_, button) => Binding::GamepadButton { button },
            Self::Stick(_, axis, direction) => Binding::GamepadAxis { axis, direction },
        }
    }

    /// The pad that pressed it; `None` for a key.
    fn pad(self) -> Option<GamepadId> {
        match self {
            Self::Key(_) => None,
            Self::Pad(pad, _) | Self::Stick(pad, ..) => Some(pad),
        }
    }
}

/// Navigation repeat and double-click timing (`D-149`), on the real clock,
/// the wheel's step and the drag threshold (M57).
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
    /// UI units one wheel line moves a scroll panel; 40 (M57). A value that
    /// is not finite and above 0 acts as 40.
    pub scroll_line: f32,
    /// How far, in UI units, the pointer must move from a primary press on
    /// a drag handle before the drag moves its root; 4, the double-click
    /// distance (M57). A value that is not finite and above 0 acts as 4.
    pub drag_threshold: f32,
}

/// [`UiRouteSettings::scroll_line`]'s default.
const SCROLL_LINE: f32 = 40.0;

/// [`UiRouteSettings::drag_threshold`]'s default.
const DRAG_THRESHOLD: f32 = 4.0;

impl Default for UiRouteSettings {
    fn default() -> Self {
        Self {
            repeat_delay: 0.5,
            repeat_interval: 0.1,
            double_click_time: 0.5,
            double_click_distance: 4.0,
            scroll_line: SCROLL_LINE,
            drag_threshold: DRAG_THRESHOLD,
        }
    }
}

impl UiRouteSettings {
    /// The wheel line in use: [`Self::scroll_line`] when it is finite and
    /// above 0, else its default.
    fn line(&self) -> f32 {
        if self.scroll_line.is_finite() && self.scroll_line > 0.0 {
            self.scroll_line
        } else {
            SCROLL_LINE
        }
    }

    /// The drag threshold in use: [`Self::drag_threshold`] when it is
    /// finite and above 0, else its default.
    fn threshold(&self) -> f32 {
        if self.drag_threshold.is_finite() && self.drag_threshold > 0.0 {
            self.drag_threshold
        } else {
            DRAG_THRESHOLD
        }
    }
}

/// A primary press on a drag handle (M57): the root it moves, the press's
/// point in UI units and the root's top-left then in the root's (M58), and
/// whether the pointer has passed the threshold.
#[derive(Debug, Clone, Copy)]
struct Drag {
    root: WidgetId,
    from: Vec2,
    start: Vec2,
    moving: bool,
}

/// A held direction and when it next moves focus.
#[derive(Debug, Clone, Copy)]
struct Repeat {
    dir: NavDirection,
    input: NavInput,
    /// Router clock seconds.
    next: f64,
}

/// The routing state: a `World` resource that [`ui_route_system`] updates
/// once a frame, in `pre_update`, and the game reads (`D-149`).
///
/// It keeps widget IDs across frames (the press, hover and focus), and IDs
/// are the `UiTree`'s: a game that replaces the `UiTree` resource while a
/// press is held can send `Released` and `Activated` to the new tree's
/// widget in that slot. Replace the tree between presses, or insert a fresh
/// router with it, carrying over [`UiRouter::settings`].
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
    /// The captured widget's drag, when it is a drag handle (M57).
    drag: Option<Drag>,
    /// Other buttons' presses on targets, until their releases.
    presses: Vec<(MouseButton, WidgetId)>,
    /// Keys, pad buttons, stick directions and mouse buttons whose presses
    /// the UI took: their releases are its.
    ui_held: Vec<NavInput>,
    ui_buttons: Vec<MouseButton>,
    /// Pads' axes whose stick direction the UI holds: the routed view reads
    /// them as 0 until that direction's release.
    withheld: Vec<(GamepadId, GamepadAxis)>,
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
    /// The `ui_*` actions of each binding pressed in this routing's stream.
    nav_actions: Vec<(Binding, u8)>,
    /// The widget a pending capture is for (M56).
    capture: Option<WidgetId>,
    /// Events from between routings (a capture cancelled or replaced),
    /// sent at the next one.
    pending_events: Vec<UiEvent>,
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

    /// The device of the last key, button, stick or wheel input routed;
    /// `Keyboard` before any.
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
    /// root is shown or a capture is pending, every key press is the UI's;
    /// while a widget holds visible focus, the presses of keys bound to the
    /// `ui_*` actions are.
    #[must_use]
    pub fn wants_keyboard(&self) -> bool {
        self.wants_keyboard
    }

    /// Captures the next key, mouse button, pad button or stick direction
    /// press for `widget`, from the next routing (M56): until then every
    /// such press is the UI's ([`Self::wants_keyboard`] holds) and none
    /// navigates; the first that is not an OS repeat ends the capture with
    /// [`UiEvent::InputCaptured`], and its release stays the UI's. Escape
    /// is captured like any key (Q9); the wheel is taken, neither captured
    /// nor routed (Q10). A capture already pending ends with
    /// [`UiEvent::CaptureCancelled`], as one does on window focus loss or
    /// when the widget is removed, hidden, disabled or frozen.
    pub fn capture_next_input(&mut self, widget: WidgetId) {
        if let Some(previous) = self.capture.replace(widget) {
            self.pending_events
                .push(UiEvent::CaptureCancelled { widget: previous });
        }
    }

    /// Ends a pending capture; its [`UiEvent::CaptureCancelled`] comes at
    /// the next routing. Does nothing when none is pending.
    pub fn cancel_capture(&mut self) {
        if let Some(widget) = self.capture.take() {
            self.pending_events
                .push(UiEvent::CaptureCancelled { widget });
        }
    }

    /// The widget a pending capture is for.
    #[must_use]
    pub fn capturing(&self) -> Option<WidgetId> {
        self.capture
    }

    /// The system cursor the window shows: the nearest one set
    /// ([`UiTree::set_cursor`]) on the captured widget or its ancestors,
    /// else on the widget under the pointer or its ancestors, a drag handle
    /// with none set counting as [`CursorIcon::Move`] (M57), else
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
        let mut nav_actions = std::mem::take(&mut self.nav_actions);
        nav_actions.clear();
        let presses = stream.iter().filter_map(|event| nav_press(event.input));
        if let Some(map) = world.get_resource::<ActionMap>() {
            for nav in presses {
                let binding = nav.binding();
                if !nav_actions.iter().any(|(seen, _)| *seen == binding) {
                    nav_actions.push((binding, ui_actions(map, binding)));
                }
            }
        }
        self.events.clear();
        self.events.append(&mut self.pending_events);
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
            nav_actions: &nav_actions,
            scrolled: Vec::new(),
        }
        .run(&stream);
        self.resolve_prompts(world);
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
        self.nav_actions = nav_actions;
    }

    /// Rebuilds the prompts whose device kind or bindings moved (M56). The
    /// prompts leave the tree while the action map is read.
    fn resolve_prompts(&self, world: &mut World) {
        let Some(tree) = world.get_resource_mut::<UiTree>() else {
            return;
        };
        if tree.prompts.is_empty() {
            return;
        }
        let mut prompts = tree.take_prompts();
        let mut texts = Vec::new();
        prompt::resolve_prompts(
            &mut prompts,
            world.get_resource::<ActionMap>(),
            self.last_device,
            &mut texts,
        );
        if let Some(tree) = world.get_resource_mut::<UiTree>() {
            tree.restore_prompts(prompts, texts);
        }
    }
}

/// Routes the frame's input through the UI (`D-149`), once a frame in
/// `pre_update` before the state dispatcher, so a state's `update` reads
/// this frame's routing (M56); the umbrella's `UiPlugin` registers it as
/// `ui_route`. Reads [`InputState::raw_events`] in arrival order against
/// the [`UiTree`]'s last committed layout, pointer positions divided by
/// [`ScaleFactor::get`], timers on [`Time::real_delta`]; updates the tree's
/// hover, press and focus and the [`UiRouter`]; resolves the tree's prompts
/// against the [`ActionMap`] and the last device; sends [`UiEvent`]s when
/// `EventQueue<UiEvent>` is registered; and feeds [`RoutedInput`], after
/// its `begin_frame`, the input the UI did not take. A world without a
/// `UiRouter` gets one; without a `UiTree`, every input is the game's.
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

/// The press `input` is, when it may navigate: a key press that is not an
/// OS repeat, a pad button press or a stick direction press.
fn nav_press(input: RawInput) -> Option<NavInput> {
    match input {
        RawInput::Key {
            code,
            pressed: true,
            repeat: false,
            ..
        } => Some(NavInput::Key(code)),
        RawInput::GamepadButton {
            pad,
            button,
            pressed: true,
        } => Some(NavInput::Pad(pad, button)),
        RawInput::GamepadStick {
            pad,
            axis,
            direction,
            pressed: true,
        } => Some(NavInput::Stick(pad, axis, direction)),
        _ => None,
    }
}

/// The `ui_*` actions `binding` is in, as bits.
fn ui_actions(map: &ActionMap, binding: Binding) -> u8 {
    ACTIONS
        .iter()
        .filter(|(action, _)| map.bindings(action).contains(&binding))
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
        RawInput::GamepadButton {
            pad,
            button,
            pressed: true,
        } => routed.gamepad_button_down(pad, button),
        RawInput::GamepadButton {
            pad,
            button,
            pressed: false,
        } => routed.gamepad_button_up(pad, button),
        RawInput::GamepadAxis { pad, axis, value } => routed.set_gamepad_axis(pad, axis, value),
        // The routed view derives its stick edges from the values it is fed.
        RawInput::GamepadStick { .. } => {}
        RawInput::Gamepad {
            pad,
            connected: true,
        } => routed.gamepad_connected(pad),
        RawInput::Gamepad {
            pad,
            connected: false,
        } => routed.gamepad_disconnected(pad),
    }
}

/// One routing's borrowed state.
struct Routing<'a> {
    router: &'a mut UiRouter,
    tree: &'a mut UiTree,
    scale: f32,
    hide: bool,
    nav_actions: &'a [(Binding, u8)],
    /// The top modal root that takes input; fixed for the routing, since
    /// only game code shows or hides roots.
    modal: Option<WidgetId>,
    /// The scroll panels the wheel moved, in the order it first moved them.
    scrolled: Vec<WidgetId>,
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
            .set_focus_visible(self.router.last_device != InputDevice::Pointer);
        self.sync_focus();
        self.check_presses();
        self.check_capture();
        self.focus_into_modal();
        self.update_hover();
        for event in stream {
            self.event(event.input);
        }
        self.tick_repeat();
        self.sync_focus();
        for widget in std::mem::take(&mut self.scrolled) {
            if let Some(offset) = self.tree.scroll(widget) {
                self.router
                    .events
                    .push(UiEvent::Scrolled { widget, offset });
            }
        }
        self.finish();
    }

    fn event(&mut self, input: RawInput) {
        match input {
            RawInput::PointerMoved { x, y } => {
                self.router.pointer = Some(Vec2::new(x, y));
                self.update_hover();
                if let Some(captured) = self.router.captured {
                    self.drag_to_pointer();
                    self.slide_to_pointer(captured);
                }
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
            RawInput::Wheel { delta } => {
                if self.router.capture.is_some() {
                    // Taken: neither captured nor routed (Q10).
                    return;
                }
                self.set_device(InputDevice::Pointer);
                if self.pointer_takes() {
                    self.wheel(delta);
                } else {
                    self.router.routed.push(input);
                }
            }
            RawInput::Key {
                code, repeat: true, ..
            } => {
                if self.router.capture.is_none()
                    && !self.router.ui_held.contains(&NavInput::Key(code))
                {
                    self.router.routed.push(input);
                }
            }
            RawInput::Key {
                code,
                pressed: true,
                modifiers,
                ..
            } => {
                self.nav_down(
                    NavInput::Key(code),
                    modifiers.shift(),
                    input,
                    InputDevice::Keyboard,
                );
            }
            RawInput::Key {
                code,
                pressed: false,
                ..
            } => self.nav_up(NavInput::Key(code), Some(input)),
            RawInput::GamepadButton {
                pad,
                button,
                pressed: true,
            } => {
                self.nav_down(
                    NavInput::Pad(pad, button),
                    false,
                    input,
                    InputDevice::Gamepad,
                );
            }
            RawInput::GamepadButton {
                pad,
                button,
                pressed: false,
            } => self.nav_up(NavInput::Pad(pad, button), Some(input)),
            RawInput::GamepadStick {
                pad,
                axis,
                direction,
                pressed: true,
            } => {
                let nav = NavInput::Stick(pad, axis, direction);
                if self.nav_down(nav, false, input, InputDevice::Gamepad) {
                    // The routed view reads the axis as 0 while the UI holds
                    // the direction.
                    self.router.withheld.push((pad, axis));
                    self.router.routed.push(RawInput::GamepadAxis {
                        pad,
                        axis,
                        value: 0.0,
                    });
                }
            }
            RawInput::GamepadStick {
                pad,
                axis,
                direction,
                pressed: false,
            } => {
                if self
                    .router
                    .ui_held
                    .contains(&NavInput::Stick(pad, axis, direction))
                {
                    self.router.withheld.retain(|held| *held != (pad, axis));
                }
                self.nav_up(NavInput::Stick(pad, axis, direction), None);
            }
            RawInput::GamepadAxis { pad, axis, .. } => {
                if !self.router.withheld.contains(&(pad, axis)) {
                    self.router.routed.push(input);
                }
            }
            RawInput::Gamepad { pad, connected } => {
                if !connected {
                    self.pad_lost(pad);
                }
                self.router.routed.push(input);
            }
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
    /// interactive or drag handle ancestor-or-self that is enabled, its
    /// ancestors too; no target in a frozen root.
    fn hit(&self) -> (Option<WidgetId>, Option<WidgetId>) {
        let Some(point) = self.point() else {
            return (None, None);
        };
        let hit = self.tree.hit_test_layers(point, self.hide);
        if hit
            .and_then(|hit| self.tree.root_of(hit))
            .is_some_and(|root| self.tree.frozen(root))
        {
            return (hit, None);
        }
        let mut target = hit;
        while let Some(id) = target {
            if (self.tree.interactive(id) || self.tree.drag_handle(id))
                && self.tree.is_enabled_in_tree(id)
            {
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
        if self.capture_press(Binding::Mouse { button }) {
            self.router.ui_buttons.push(button);
            return;
        }
        if !self.pointer_takes() {
            self.router.routed.push(RawInput::MouseButton {
                button,
                pressed: true,
            });
            return;
        }
        self.router.ui_buttons.push(button);
        if button == MouseButton::Left {
            self.raise_window();
        }
        let Some(widget) = self.hit().1 else {
            return;
        };
        self.router.events.push(UiEvent::Pressed { widget, button });
        if button == MouseButton::Left {
            self.router.captured = Some(widget);
            self.start_drag(widget);
            self.tree.focus(widget);
            self.sync_focus();
            self.update_hover();
            self.slide_to_pointer(widget);
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
                if !self.end_drag() && self.router.hovered == Some(widget) {
                    self.activate(widget);
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

    /// A primary press the UI takes over a `Role::Window` root that is not
    /// frozen raises it over its layer's other roots (M58), before the press
    /// is handled, so the raised window takes it.
    fn raise_window(&mut self) {
        let Some(root) = self.hit().0.and_then(|hit| self.tree.root_of(hit)) else {
            return;
        };
        if self.tree.role(root) == Some(Role::Window) && !self.tree.frozen(root) {
            self.tree.raise(root);
        }
    }

    /// How many UI units one unit of `id`'s root spans: the debug scale of
    /// the last layout under a debug root (M58), else 1.
    fn root_scale(&self, id: WidgetId) -> f32 {
        self.tree
            .layer(id)
            .map_or(1.0, |layer| self.tree.layer_scale(layer))
    }

    /// The wheel over the UI (M57): moves the innermost scroll panel at or
    /// above the widget under the pointer that can move that way and takes
    /// input, a line by `scroll_line` units of the panel and a pixel delta
    /// divided by the scale and, under a debug root, the debug scale (M58);
    /// a positive `y` moves the content down. A delta that is not finite
    /// moves nothing.
    fn wheel(&mut self, delta: WheelDelta) {
        let hit = self.hit().0;
        let y = match delta {
            WheelDelta::Lines(_, y) => y * self.router.settings.line(),
            WheelDelta::Pixels(_, y) => {
                y / (self.scale * hit.map_or(1.0, |hit| self.root_scale(hit)))
            }
        };
        if !y.is_finite() || y == 0.0 {
            return;
        }
        let mut current = hit;
        while let Some(id) = current {
            if self.takes_input(id) && self.tree.scroll_by(id, -y).is_some() {
                if !self.scrolled.contains(&id) {
                    self.scrolled.push(id);
                }
                self.update_hover();
                return;
            }
            current = self.tree.parent(id);
        }
    }

    /// A primary press on `widget`: a drag handle starts a drag of its
    /// root, from the pointer and the root's top-left at the last layout.
    fn start_drag(&mut self, widget: WidgetId) {
        self.router.drag = None;
        if !self.tree.drag_handle(widget) {
            return;
        }
        let (Some(from), Some(root)) = (self.point(), self.tree.root_of(widget)) else {
            return;
        };
        if let Some(start) = self.tree.window_position(root) {
            self.router.drag = Some(Drag {
                root,
                from,
                start,
                moving: false,
            });
        }
    }

    /// Moves a drag's root by the pointer's motion since the press, once
    /// that motion is past the threshold; a debug root moves by it divided
    /// by the debug scale, in its own units (M58).
    fn drag_to_pointer(&mut self) {
        let (Some(mut drag), Some(point)) = (self.router.drag, self.point()) else {
            return;
        };
        if !drag.moving {
            if point.distance(drag.from) <= self.router.settings.threshold() {
                return;
            }
            drag.moving = true;
            self.router.drag = Some(drag);
        }
        let motion = (point - drag.from) / self.root_scale(drag.root);
        self.tree
            .set_window_position(drag.root, drag.start + motion);
    }

    /// Ends the drag of a press that ended: a root it moved, still in the
    /// tree, reports `WindowMoved`. Returns whether the press dragged.
    fn end_drag(&mut self) -> bool {
        let Some(drag) = self.router.drag.take() else {
            return false;
        };
        if drag.moving && self.tree.contains(drag.root) {
            self.router
                .events
                .push(UiEvent::WindowMoved { window: drag.root });
        }
        drag.moving
    }

    /// Sets a slider's value from the pointer, past either end too, and
    /// reports a change; anything else, or no pointer, changes nothing. The
    /// pointer maps onto the box the slider paints in.
    fn slide_to_pointer(&mut self, widget: WidgetId) {
        let (Some(point), Some(rect), Some(range)) = (
            self.point(),
            self.tree.visual_rect(widget),
            self.tree.slider_range(widget),
        ) else {
            return;
        };
        let value = control::pointer_value(rect, range, point.x);
        self.set_slider(widget, value);
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

    /// Ends every press without a release; a drag leaves its root where it
    /// is.
    fn cancel_presses(&mut self) {
        if let Some(widget) = self.router.captured.take() {
            self.router.events.push(UiEvent::PressCancelled { widget });
            self.end_drag();
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
            self.end_drag();
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

    /// Ends a pending capture whose widget was removed, hidden or disabled,
    /// or whose root stopped taking input or froze, since the last routing.
    fn check_capture(&mut self) {
        if let Some(widget) = self.router.capture
            && !self.takes_input(widget)
        {
            self.router.capture = None;
            self.router
                .events
                .push(UiEvent::CaptureCancelled { widget });
        }
    }

    /// Ends a pending capture with `binding`, the press it took; `false`
    /// when none is pending.
    fn capture_press(&mut self, binding: Binding) -> bool {
        let Some(widget) = self.router.capture.take() else {
            return false;
        };
        self.router
            .events
            .push(UiEvent::InputCaptured { widget, binding });
        true
    }

    fn takes_input(&self, widget: WidgetId) -> bool {
        self.tree.is_shown(widget)
            && self.tree.is_enabled_in_tree(widget)
            && self.tree.root_of(widget).is_some_and(|root| {
                self.tree.root_routes(root, self.hide) && !self.tree.frozen(root)
            })
    }

    // ---- keyboard ----

    /// The focused widget when keys act on it: in a root that takes input
    /// and is not frozen, and not under the shown modal root.
    fn focus_in_play(&self) -> Option<WidgetId> {
        let focused = self.tree.focused()?;
        let root = self.tree.root_of(focused)?;
        if !self.tree.root_routes(root, self.hide) || self.tree.frozen(root) {
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

    /// A shown modal root that is not frozen, with no focus in it or above
    /// it, focuses its first focusable widget.
    fn focus_into_modal(&mut self) {
        if let Some(modal) = self.modal
            && !self.tree.frozen(modal)
            && self.focus_in_play().is_none()
            && let Some(first) = self.tree.first_focusable(modal)
        {
            self.tree.focus(first);
            self.sync_focus();
        }
    }

    /// A key, pad button or stick direction press: the UI takes it while a
    /// modal root is shown, or while a widget holds visible focus and the
    /// press is bound to a `ui_*` action; otherwise it goes to the routed
    /// view, except a stick press, whose axis record carries it. Returns
    /// whether the UI took it.
    fn nav_down(
        &mut self,
        nav: NavInput,
        shift: bool,
        input: RawInput,
        device: InputDevice,
    ) -> bool {
        let binding = nav.binding();
        if self.router.capture.is_some() {
            self.set_device(device);
            self.capture_press(binding);
            self.router.ui_held.push(nav);
            return true;
        }
        let actions = self
            .nav_actions
            .iter()
            .find(|(seen, _)| *seen == binding)
            .map_or(0, |(_, actions)| *actions);
        let takes = self.modal.is_some()
            || (actions != 0 && self.focus_in_play().is_some() && self.tree.focus_visible());
        self.set_device(device);
        if !takes {
            if !matches!(nav, NavInput::Stick(..)) {
                self.router.routed.push(input);
            }
            return false;
        }
        self.router.ui_held.push(nav);
        if actions & (NEXT | PREV) != 0 && self.focus_in_play().is_some() {
            if actions & PREV != 0 || shift {
                self.tree.focus_prev();
            } else {
                self.tree.focus_next();
            }
            self.sync_focus();
        }
        if let Some(&(_, dir)) = DIRECTIONS.iter().find(|(bit, _)| actions & bit != 0) {
            self.navigate(dir);
            self.router.repeat = Some(Repeat {
                dir,
                input: nav,
                next: self.router.clock + f64::from(self.router.settings.repeat_delay),
            });
        }
        if actions & ACCEPT != 0
            && let Some(widget) = self.focus_in_play()
        {
            self.activate(widget);
        }
        if actions & CANCEL != 0 {
            let scope = self
                .focus_in_play()
                .and_then(|focused| self.tree.scope_of(focused))
                .or(self.modal.filter(|modal| !self.tree.frozen(*modal)));
            if let Some(scope) = scope {
                self.router.events.push(UiEvent::Cancel { scope });
            }
            if self.modal.is_none() {
                self.tree.blur();
                self.sync_focus();
            }
        }
        true
    }

    /// A release: the UI's when it took the press, else `input` goes to the
    /// routed view (a stick release has none: its axis record carries it).
    fn nav_up(&mut self, nav: NavInput, input: Option<RawInput>) {
        if self.router.repeat.is_some_and(|repeat| repeat.input == nav) {
            self.router.repeat = None;
        }
        match self.router.ui_held.iter().position(|held| *held == nav) {
            Some(at) => {
                self.router.ui_held.swap_remove(at);
            }
            None => self.router.routed.extend(input),
        }
    }

    /// A pad disconnected: the UI forgets its held presses and its withheld
    /// axes, and a repeat it drove stops.
    fn pad_lost(&mut self, pad: GamepadId) {
        self.router.ui_held.retain(|held| held.pad() != Some(pad));
        self.router.withheld.retain(|(held, _)| *held != pad);
        if self
            .router
            .repeat
            .is_some_and(|repeat| repeat.input.pad() == Some(pad))
        {
            self.router.repeat = None;
        }
    }

    /// A direction press or its repeat, while focus is in play: `Left` and
    /// `Right` step a focused slider's value, at an end changing nothing;
    /// otherwise focus moves that way.
    fn navigate(&mut self, dir: NavDirection) {
        let Some(focused) = self.focus_in_play() else {
            return;
        };
        let up = match dir {
            NavDirection::Left => Some(false),
            NavDirection::Right => Some(true),
            _ => None,
        };
        if let (Some(up), Some(range), Some(value)) = (
            up,
            self.tree.slider_range(focused),
            self.tree.slider_value(focused),
        ) {
            self.set_slider(focused, range.key_step(value, up));
            return;
        }
        self.tree.focus_direction(dir);
        self.sync_focus();
    }

    /// Moves focus, or steps a focused slider, once more for a held
    /// direction that is due, at most once a routing; a routing late by more
    /// than an interval does not catch up.
    fn tick_repeat(&mut self) {
        let Some(mut repeat) = self.router.repeat else {
            return;
        };
        if self.router.capture.is_some() {
            return;
        }
        let clock = self.router.clock;
        if clock < repeat.next {
            return;
        }
        self.navigate(repeat.dir);
        let interval = f64::from(self.router.settings.repeat_interval);
        repeat.next += interval;
        if repeat.next <= clock {
            repeat.next = clock + interval;
        }
        self.router.repeat = Some(repeat);
    }

    /// Window focus loss: ends the presses and a pending capture, forgets
    /// the UI's held keys, buttons, stick directions and withheld axes and
    /// the pointer, and clears the hover.
    fn focus_lost(&mut self) {
        self.cancel_presses();
        if let Some(widget) = self.router.capture.take() {
            self.router
                .events
                .push(UiEvent::CaptureCancelled { widget });
        }
        self.router.ui_held.clear();
        self.router.ui_buttons.clear();
        self.router.withheld.clear();
        self.router.repeat = None;
        self.router.pointer = None;
        self.update_hover();
    }

    // ---- shared ----

    /// An activation: `Activated`, then a toggle flips and reports its new
    /// state.
    fn activate(&mut self, widget: WidgetId) {
        self.router.events.push(UiEvent::Activated { widget });
        if let Some(on) = self.tree.toggled(widget) {
            self.tree.set_toggled(widget, !on);
            self.router.events.push(UiEvent::ValueChanged {
                widget,
                value: UiValue::Bool(!on),
            });
        }
    }

    /// Sets a slider's value and reports it when the stored value changed.
    fn set_slider(&mut self, widget: WidgetId, value: f32) {
        if self.tree.set_slider_value(widget, value)
            && let Some(value) = self.tree.slider_value(widget)
        {
            self.router.events.push(UiEvent::ValueChanged {
                widget,
                value: UiValue::Number(value),
            });
        }
    }

    fn set_device(&mut self, device: InputDevice) {
        self.router.last_device = device;
        self.tree.set_focus_visible(device != InputDevice::Pointer);
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
        self.router.wants_keyboard = self.modal.is_some()
            || self.router.capture.is_some()
            || (self.focus_in_play().is_some() && self.tree.focus_visible());
        let mut source = self.router.captured.or(hit);
        let mut cursor = None;
        while let Some(id) = source {
            cursor = self
                .tree
                .cursor(id)
                .or_else(|| self.tree.drag_handle(id).then_some(CursorIcon::Move));
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
