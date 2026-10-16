use super::*;
use crate::input::Modifiers;
use crate::ui::{
    Anchor, FixedAdvanceMeasure, LayoutStyle, Rect, Sizing, SliderRange, UiLayer, Visibility,
};

/// The logical viewport every rig lays out in.
const VIEWPORT: Vec2 = Vec2::new(400.0, 300.0);
/// One frame's real seconds: 1/32 is exact in binary, so 0.5 s is 16
/// frames and no timer edge falls on a rounding error.
const DT: f32 = 1.0 / 32.0;
const LEFT: MouseButton = MouseButton::Left;

fn at(x: f32, y: f32, w: f32, h: f32) -> LayoutStyle {
    LayoutStyle::default()
        .with_size(Sizing::Px(w), Sizing::Px(h))
        .anchored(Anchor::TopStart, Vec2::new(x, y))
}

fn activations(events: &[UiEvent]) -> Vec<WidgetId> {
    events
        .iter()
        .filter_map(|event| match event {
            UiEvent::Activated { widget } => Some(*widget),
            _ => None,
        })
        .collect()
}

/// A hand-built world with every resource the router reads, driven a frame
/// at a time as the app does: raw input, the clock, the routing, then the
/// frame's layout.
struct Rig {
    world: World,
}

impl Rig {
    /// A full-viewport `Game` root, laid out.
    fn empty() -> (Self, WidgetId) {
        let mut world = World::new();
        world.insert_resource(InputState::new());
        world.insert_resource(ActionMap::default_map());
        world.insert_resource(Time::new());
        world.insert_resource(ScaleFactor::default());
        world.insert_resource(UiRouter::default());
        world.insert_resource(RoutedInput::default());
        world.register_event::<UiEvent>();
        let mut tree = UiTree::new();
        let root = tree.add_root(
            UiLayer::Game,
            LayoutStyle::default().with_size(Sizing::Px(400.0), Sizing::Px(300.0)),
        );
        world.insert_resource(tree);
        let mut rig = Self { world };
        rig.layout();
        (rig, root)
    }

    /// The root with buttons `a` at (10, 10) and `b` at (10, 60), 100 by
    /// 30 each.
    fn new() -> (Self, WidgetId, WidgetId, WidgetId) {
        let (mut rig, root) = Self::empty();
        let a = rig.place(root, at(10.0, 10.0, 100.0, 30.0));
        let b = rig.place(root, at(10.0, 60.0, 100.0, 30.0));
        rig.layout();
        (rig, root, a, b)
    }

    fn place(&mut self, parent: WidgetId, style: LayoutStyle) -> WidgetId {
        let tree = self.tree_mut();
        let button = tree.button(parent, "x");
        tree.set_layout_style(button, style);
        button
    }

    fn layout(&mut self) {
        self.tree_mut()
            .layout(VIEWPORT, &mut FixedAdvanceMeasure::default());
    }

    fn frame(&mut self, feed: impl FnOnce(&mut InputState)) -> Vec<UiEvent> {
        self.frame_dt(DT, feed)
    }

    /// One frame of `dt` real seconds: the queues rotate, `feed` gives the
    /// raw input, the clock advances, the UI routes, the tree lays out.
    /// Returns the routing's events.
    fn frame_dt(&mut self, dt: f32, feed: impl FnOnce(&mut InputState)) -> Vec<UiEvent> {
        self.world.flush_events();
        let input = self.world.resource_mut::<InputState>();
        input.begin_frame();
        feed(input);
        self.world.resource_mut::<Time>().advance_frame(dt);
        ui_route_system(&mut self.world);
        self.layout();
        self.queue().iter_current().copied().collect()
    }

    /// A widget's centre in UI units, physical pixels at scale 1.
    fn centre(&self, id: WidgetId) -> Vec2 {
        let rect = self.tree().rect(id).expect("laid out");
        (rect.min + rect.max) * 0.5
    }

    fn click(&mut self, id: WidgetId) -> Vec<UiEvent> {
        let point = self.centre(id);
        self.click_px(point)
    }

    /// A move, a press and a release at `point` in physical pixels.
    fn click_px(&mut self, point: Vec2) -> Vec<UiEvent> {
        self.frame(|input| {
            input.update_cursor_position(point.x, point.y);
            input.mouse_down(LEFT);
            input.mouse_up(LEFT);
        })
    }

    fn tree(&self) -> &UiTree {
        self.world.resource::<UiTree>()
    }

    fn tree_mut(&mut self) -> &mut UiTree {
        self.world.resource_mut::<UiTree>()
    }

    fn router(&self) -> &UiRouter {
        self.world.resource::<UiRouter>()
    }

    fn routed(&self) -> &RoutedInput {
        self.world.resource::<RoutedInput>()
    }

    fn queue(&self) -> &EventQueue<UiEvent> {
        self.world.resource::<EventQueue<UiEvent>>()
    }

    /// Whether the routed view pressed `action` this frame.
    fn game_pressed(&self, action: &str) -> bool {
        self.world
            .resource::<ActionMap>()
            .just_pressed(self.routed(), action)
    }
}

#[test]
fn a_move_press_and_release_in_one_stream_click_where_the_move_put_the_pointer() {
    let (mut rig, _root, a, b) = Rig::new();
    let at_b = rig.centre(b);
    let events = rig.click_px(at_b);
    assert_eq!(
        events,
        [
            UiEvent::PointerEntered { widget: b },
            UiEvent::Pressed {
                widget: b,
                button: LEFT
            },
            UiEvent::FocusGained { widget: b },
            UiEvent::Released {
                widget: b,
                button: LEFT
            },
            UiEvent::Activated { widget: b },
        ]
    );
    assert!(!activations(&events).contains(&a));
    assert_eq!(rig.tree().focused(), Some(b), "a click focuses its target");
    assert_eq!(rig.tree().hovered(), Some(b));
    assert_eq!(rig.tree().pressed(), None);
    let routed = rig.routed();
    assert!(
        !routed.mouse_just_pressed(LEFT) && !routed.mouse_just_released(LEFT),
        "the click is the UI's"
    );
    assert_eq!(
        routed.cursor_position(),
        Some((at_b.x, at_b.y)),
        "moves always reach the routed view"
    );
    assert!(rig.router().pointer_over_ui());
}

#[test]
fn a_release_off_the_captured_widget_releases_without_activating() {
    let (mut rig, _root, a, b) = Rig::new();
    let (at_a, at_b) = (rig.centre(a), rig.centre(b));
    rig.frame(|input| {
        input.update_cursor_position(at_b.x, at_b.y);
        input.mouse_down(LEFT);
    });
    assert_eq!(rig.tree().pressed(), Some(b));
    let events = rig.frame(|input| input.update_cursor_position(at_a.x, at_a.y));
    assert_eq!(
        events,
        [UiEvent::PointerLeft { widget: b }],
        "only the captured widget hovers"
    );
    assert_eq!(rig.tree().hovered(), None);
    assert_eq!(rig.tree().pressed(), None, "pressed only while over it");
    rig.frame(|input| input.update_cursor_position(at_b.x, at_b.y));
    assert_eq!(rig.tree().pressed(), Some(b));
    rig.frame(|input| input.update_cursor_position(at_a.x, at_a.y));
    let events = rig.frame(|input| input.mouse_up(LEFT));
    assert_eq!(
        events,
        [
            UiEvent::Released {
                widget: b,
                button: LEFT
            },
            UiEvent::PointerEntered { widget: a },
        ]
    );
    assert!(
        !rig.routed().mouse_just_released(LEFT),
        "the UI got the press, so the release"
    );
}

#[test]
fn another_button_presses_and_releases_its_target_and_does_nothing_else() {
    let (mut rig, _root, a, _b) = Rig::new();
    let at_a = rig.centre(a);
    let right = MouseButton::Right;
    let events = rig.frame(|input| {
        input.update_cursor_position(at_a.x, at_a.y);
        input.mouse_down(right);
    });
    assert_eq!(
        events,
        [
            UiEvent::PointerEntered { widget: a },
            UiEvent::Pressed {
                widget: a,
                button: right
            },
        ]
    );
    assert_eq!(rig.tree().focused(), None, "no capture and no focus");
    assert_eq!(rig.tree().pressed(), None);
    let events = rig.frame(|input| input.mouse_up(right));
    assert_eq!(
        events,
        [UiEvent::Released {
            widget: a,
            button: right
        }]
    );
}

#[test]
fn a_press_ends_cancelled_when_its_widget_hides_or_the_pointer_leaves() {
    let (mut rig, _root, a, b) = Rig::new();
    let at_a = rig.centre(a);
    rig.frame(|input| {
        input.update_cursor_position(at_a.x, at_a.y);
        input.mouse_down(LEFT);
    });
    rig.tree_mut().set_visibility(a, Visibility::Hidden);
    let events = rig.frame(|_| {});
    assert_eq!(
        events,
        [
            UiEvent::FocusLost { widget: a },
            UiEvent::PressCancelled { widget: a },
            UiEvent::PointerLeft { widget: a },
        ]
    );
    let events = rig.frame(|input| input.mouse_up(LEFT));
    assert!(events.is_empty(), "the cancelled press's release is silent");
    assert!(!rig.routed().mouse_just_released(LEFT));

    let at_b = rig.centre(b);
    rig.frame(|input| {
        input.update_cursor_position(at_b.x, at_b.y);
        input.mouse_down(LEFT);
    });
    let events = rig.frame(InputState::pointer_left);
    assert_eq!(
        events,
        [
            UiEvent::PressCancelled { widget: b },
            UiEvent::PointerLeft { widget: b },
        ]
    );
    assert_eq!(rig.tree().pressed(), None);
    assert!(!rig.router().pointer_over_ui());
}

#[test]
fn focus_loss_cancels_the_press_clears_the_hover_and_releases_both_sides() {
    let (mut rig, _root, a, b) = Rig::new();
    rig.tree_mut().focus(a);
    let at_b = rig.centre(b);
    rig.frame(|input| {
        input.key_down(KeyCode::ArrowDown);
        input.key_down(KeyCode::KeyW);
        input.update_cursor_position(at_b.x, at_b.y);
        input.mouse_down(LEFT);
    });
    assert_eq!(rig.tree().focused(), Some(b), "the UI took the arrow");
    assert!(rig.routed().is_pressed(KeyCode::KeyW));
    assert!(!rig.routed().is_pressed(KeyCode::ArrowDown));
    assert_eq!(rig.tree().pressed(), Some(b));

    let events = rig.frame(|input| input.set_focused(false));
    assert_eq!(
        events,
        [
            UiEvent::PressCancelled { widget: b },
            UiEvent::PointerLeft { widget: b },
        ]
    );
    assert_eq!((rig.tree().hovered(), rig.tree().pressed()), (None, None));
    let routed = rig.routed();
    assert!(!routed.is_pressed(KeyCode::KeyW));
    assert!(routed.just_released(KeyCode::KeyW), "with a release edge");
    assert!(!routed.focused());
    let router = rig.router();
    assert!(
        router.ui_held.is_empty() && router.ui_buttons.is_empty(),
        "the UI's held keys and buttons are forgotten"
    );
    rig.frame(|_| {});
    assert_eq!(
        rig.tree().hovered(),
        None,
        "nor does the next routing hover from the old position"
    );
}

#[test]
fn one_click_or_accept_press_activates_once() {
    let (mut rig, _root, _a, b) = Rig::new();
    rig.click(b);
    assert!(rig.queue().activated(b));
    rig.frame(|_| {});
    assert!(
        !rig.queue().activated(b),
        "activated() sees the click in one frame"
    );
    assert_eq!(
        rig.queue()
            .iter()
            .filter(|event| **event == UiEvent::Activated { widget: b })
            .count(),
        1,
        "where iter() sees it in a second"
    );

    rig.frame(|input| input.key_down(KeyCode::KeyW));
    assert!(rig.tree().focus_visible(), "a game key shows focus");
    let events = rig.frame(|input| input.key_down(KeyCode::Enter));
    assert_eq!(activations(&events), [b]);
    let events = rig.frame(|input| input.key_down(KeyCode::Enter));
    assert!(activations(&events).is_empty(), "an OS repeat");
    let events = rig.frame(|_| {});
    assert!(activations(&events).is_empty(), "a held key");
    rig.frame(|input| input.key_up(KeyCode::Enter));
    let events = rig.frame(|input| input.key_down(KeyCode::Space));
    assert_eq!(activations(&events), [b], "Space accepts too");
}

#[test]
fn a_held_direction_moves_focus_then_repeats_after_half_a_second_every_tenth() {
    let (mut rig, root) = Rig::empty();
    let column: Vec<WidgetId> = (0..12)
        .map(|i| rig.place(root, at(10.0, 24.0 * i as f32, 100.0, 20.0)))
        .collect();
    rig.layout();
    let index = |rig: &Rig| {
        column
            .iter()
            .position(|id| rig.tree().focused() == Some(*id))
            .expect("focus in the column")
    };
    rig.tree_mut().focus(column[0]);
    rig.frame(|input| input.key_down(KeyCode::ArrowDown));
    assert_eq!(index(&rig), 1, "the press moves once");
    assert!(!rig.routed().is_pressed(KeyCode::ArrowDown));

    // 0.5 s is 16 frames; then 0.6, 0.7 and 0.8 s end in frames 20, 23
    // and 26 of 1/32 s.
    let mut moved = Vec::new();
    for frame in 1..=26 {
        let before = rig.tree().focused();
        rig.frame(|_| {});
        if rig.tree().focused() != before {
            moved.push(frame);
        }
    }
    assert_eq!(moved, [16, 20, 23, 26]);
    assert_eq!(index(&rig), 5);

    rig.frame_dt(0.35, |_| {});
    assert_eq!(index(&rig), 6, "at most once a routing");
    rig.frame(|_| {});
    assert_eq!(index(&rig), 6, "and no catching up after it");

    rig.frame(|input| input.key_up(KeyCode::ArrowDown));
    for _ in 0..40 {
        rig.frame(|_| {});
    }
    assert_eq!(index(&rig), 6, "the release stops it");
    assert!(!rig.routed().just_released(KeyCode::ArrowDown));
}

#[test]
fn tab_moves_in_tab_order_and_shift_or_a_bound_ui_prev_goes_back() {
    let (mut rig, _root, a, b) = Rig::new();
    rig.tree_mut().focus(a);
    rig.frame(|input| input.key_down(KeyCode::Tab));
    assert_eq!(rig.tree().focused(), Some(b));
    rig.frame(|input| {
        input.key_up(KeyCode::Tab);
        input.set_modifiers(Modifiers::SHIFT);
        input.key_down(KeyCode::Tab);
    });
    assert_eq!(rig.tree().focused(), Some(a), "Shift with ui_next");
    rig.frame(|input| {
        input.key_up(KeyCode::Tab);
        input.set_modifiers(Modifiers::empty());
    });
    rig.world.resource_mut::<ActionMap>().replace_bindings(
        UI_PREV,
        vec![Binding::Key {
            code: KeyCode::KeyQ,
        }],
    );
    rig.frame(|input| input.key_down(KeyCode::KeyQ));
    assert_eq!(rig.tree().focused(), Some(b), "ui_prev wraps backwards");
    assert!(!rig.routed().is_pressed(KeyCode::KeyQ));
}

#[test]
fn a_held_key_releases_to_the_side_that_got_its_press() {
    let (mut rig, _root, a, _b) = Rig::new();
    rig.click(a);
    assert!(!rig.tree().focus_visible());
    rig.frame(|input| input.key_down(KeyCode::Space));
    assert!(
        rig.routed().just_pressed(KeyCode::Space),
        "hidden focus: the game's"
    );
    assert!(rig.tree().focus_visible(), "the press shows focus");
    rig.frame(|input| input.key_up(KeyCode::Space));
    assert!(
        rig.routed().just_released(KeyCode::Space),
        "its release follows its press"
    );

    let events = rig.frame(|input| input.key_down(KeyCode::Enter));
    assert_eq!(activations(&events), [a]);
    assert!(!rig.routed().just_pressed(KeyCode::Enter));
    rig.frame(|input| input.key_up(KeyCode::Enter));
    assert!(
        !rig.routed().just_released(KeyCode::Enter),
        "one the UI took never reaches the routed view"
    );
}

#[test]
fn under_a_shown_modal_every_press_is_the_uis_and_a_debug_root_above_still_hits() {
    let (mut rig, _root, a, _b) = Rig::new();
    let popup = rig
        .tree_mut()
        .add_root(UiLayer::GamePopup, at(150.0, 100.0, 200.0, 100.0));
    rig.tree_mut().set_modal(popup, true);
    let ok = rig.place(popup, at(10.0, 10.0, 80.0, 30.0));
    let debug = rig
        .tree_mut()
        .add_root(UiLayer::Debug, at(300.0, 250.0, 100.0, 50.0));
    let tool = rig.place(debug, at(0.0, 0.0, 100.0, 50.0));
    rig.layout();

    let events = rig.frame(|input| input.key_down(KeyCode::KeyW));
    assert_eq!(
        events,
        [UiEvent::FocusGained { widget: ok }],
        "the modal takes focus on its first focusable widget"
    );
    assert!(
        !rig.routed().is_pressed(KeyCode::KeyW),
        "a key no action names is the UI's"
    );
    assert!(rig.router().wants_keyboard());

    let at_a = rig.centre(a);
    let events = rig.click_px(at_a);
    assert!(
        events.is_empty(),
        "the widget under the modal is not hit: {events:?}"
    );
    assert!(
        !rig.routed().mouse_just_pressed(LEFT),
        "and the click is the UI's"
    );
    assert!(rig.router().pointer_over_ui());

    let events = rig.click(tool);
    assert_eq!(
        activations(&events),
        [tool],
        "a Debug root above it still hits"
    );
    assert_eq!(rig.tree().focused(), Some(tool));
    rig.frame(|_| {});
    assert_eq!(
        rig.tree().focused(),
        Some(tool),
        "focus above the modal stays"
    );
}

#[test]
fn escape_with_visible_focus_cancels_and_clears_it_and_without_focus_is_the_games() {
    let (mut rig, root, a, _b) = Rig::new();
    rig.tree_mut().focus(a);
    let events = rig.frame(|input| input.key_down(KeyCode::Escape));
    assert!(events.contains(&UiEvent::Cancel { scope: root }));
    assert_eq!(
        rig.tree().focused(),
        None,
        "outside a modal it clears focus"
    );
    assert!(!rig.game_pressed("engine_exit"), "the UI took it");
    rig.frame(|input| input.key_up(KeyCode::Escape));
    let events = rig.frame(|input| input.key_down(KeyCode::Escape));
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, UiEvent::Cancel { .. }))
    );
    assert!(rig.game_pressed("engine_exit"), "without focus: the game's");

    // Under a modal Escape cancels the modal and keeps focus.
    let popup = rig
        .tree_mut()
        .add_root(UiLayer::GamePopup, at(100.0, 100.0, 200.0, 100.0));
    rig.tree_mut().set_modal(popup, true);
    let ok = rig.place(popup, at(10.0, 10.0, 80.0, 30.0));
    rig.layout();
    rig.frame(|input| input.key_up(KeyCode::Escape));
    let events = rig.frame(|input| input.key_down(KeyCode::Escape));
    assert!(events.contains(&UiEvent::Cancel { scope: popup }));
    assert_eq!(rig.tree().focused(), Some(ok));
    assert!(!rig.game_pressed("engine_exit"));
}

#[test]
fn at_scale_2_a_physical_click_lands_on_the_logical_rect() {
    let (mut rig, _root, a, b) = Rig::new();
    rig.world.resource_mut::<ScaleFactor>().set_os(2.0);
    // `a` spans (10, 10)–(110, 40) in UI units, (20, 20)–(220, 80) in
    // pixels; `b` starts at y 60, pixel 120.
    let events = rig.click_px(Vec2::new(200.0, 70.0));
    assert_eq!(activations(&events), [a]);
    let events = rig.click_px(Vec2::new(200.0, 110.0));
    assert!(activations(&events).is_empty(), "between the two");
    let events = rig.click_px(Vec2::new(100.0, 150.0));
    assert_eq!(activations(&events), [b]);
}

#[test]
fn a_second_click_on_the_same_widget_soon_and_near_is_a_double_click() {
    let (mut rig, _root, a, b) = Rig::new();
    rig.click(a);
    let events = rig.click(a);
    assert_eq!(
        &events[events.len() - 2..],
        [
            UiEvent::Activated { widget: a },
            UiEvent::DoubleClicked { widget: a },
        ]
    );
    let events = rig.click(a);
    assert!(
        !events.contains(&UiEvent::DoubleClicked { widget: a }),
        "a third click starts a pair again"
    );

    // 0.5 s apart is still one; 17 frames is past it.
    for _ in 0..15 {
        rig.frame(|_| {});
    }
    let events = rig.click(a);
    assert!(events.contains(&UiEvent::DoubleClicked { widget: a }));
    rig.click(a);
    for _ in 0..16 {
        rig.frame(|_| {});
    }
    assert!(!rig.click(a).contains(&UiEvent::DoubleClicked { widget: a }));

    let near = rig.centre(a);
    rig.click_px(near + Vec2::new(5.0, 0.0));
    assert!(
        !rig.click_px(near)
            .contains(&UiEvent::DoubleClicked { widget: a }),
        "5 units apart"
    );
    rig.click(b);
    assert!(
        !rig.click(a).contains(&UiEvent::DoubleClicked { widget: a }),
        "another widget between"
    );
}

#[test]
fn a_key_shows_focus_a_click_or_the_wheel_hides_it_and_a_move_leaves_it() {
    let (mut rig, _root, a, _b) = Rig::new();
    assert_eq!(rig.router().last_device(), InputDevice::Keyboard);
    rig.click(a);
    assert_eq!(rig.router().last_device(), InputDevice::Pointer);
    assert!(!rig.tree().focus_visible());
    assert!(!rig.router().wants_keyboard(), "hidden focus");
    rig.frame(|input| input.update_cursor_position(300.0, 200.0));
    assert!(!rig.tree().focus_visible(), "a move changes nothing");
    rig.frame(|input| input.key_down(KeyCode::KeyW));
    assert_eq!(rig.router().last_device(), InputDevice::Keyboard);
    assert!(rig.tree().focus_visible());
    assert!(rig.router().wants_keyboard());
    rig.frame(|input| input.update_cursor_position(20.0, 20.0));
    assert!(rig.tree().focus_visible(), "nor does this one");
    rig.frame(|input| input.add_scroll_line_delta(0.0, 1.0));
    assert_eq!(rig.router().last_device(), InputDevice::Pointer);
    assert!(!rig.tree().focus_visible());
    assert!(
        !rig.routed()
            .is_scroll_active(crate::input::ScrollDirection::Up),
        "the wheel over the UI is the UI's"
    );
}

#[test]
fn the_cursor_is_the_nearest_one_set_under_the_pointer_or_on_the_captured_widget() {
    let (mut rig, root, a, b) = Rig::new();
    rig.tree_mut().set_cursor(a, Some(CursorIcon::Pointer));
    rig.tree_mut().set_cursor(root, Some(CursorIcon::Move));
    let label = rig.tree().children(a)[0];
    let (at_a, at_label, at_b) = (rig.centre(a), rig.centre(label), rig.centre(b));
    assert_eq!(rig.router().cursor(), CursorIcon::Default);
    rig.frame(|input| input.update_cursor_position(at_a.x, at_a.y));
    assert_eq!(rig.router().cursor(), CursorIcon::Pointer);
    rig.frame(|input| input.update_cursor_position(at_label.x, at_label.y));
    assert_eq!(
        rig.router().cursor(),
        CursorIcon::Pointer,
        "set on the button, read through its label"
    );
    rig.frame(|input| input.update_cursor_position(at_b.x, at_b.y));
    assert_eq!(rig.router().cursor(), CursorIcon::Move, "the root's");
    rig.frame(|input| {
        input.update_cursor_position(at_a.x, at_a.y);
        input.mouse_down(LEFT);
        input.update_cursor_position(at_b.x, at_b.y);
    });
    assert_eq!(
        rig.router().cursor(),
        CursorIcon::Pointer,
        "the captured widget's"
    );
    rig.frame(|input| input.mouse_up(LEFT));
    assert_eq!(rig.router().cursor(), CursorIcon::Move);
    rig.frame(InputState::pointer_left);
    assert_eq!(rig.router().cursor(), CursorIcon::Default);
}

#[test]
fn a_focus_change_by_the_game_is_reported_at_the_next_routing() {
    let (mut rig, _root, a, b) = Rig::new();
    rig.tree_mut().focus(a);
    assert_eq!(rig.frame(|_| {}), [UiEvent::FocusGained { widget: a }]);
    rig.tree_mut().focus(b);
    assert_eq!(
        rig.frame(|_| {}),
        [
            UiEvent::FocusLost { widget: a },
            UiEvent::FocusGained { widget: b },
        ]
    );
    assert!(rig.frame(|_| {}).is_empty());
}

#[test]
fn a_root_added_since_the_last_layout_takes_no_input_until_laid_out() {
    let (mut rig, _root, a, _b) = Rig::new();
    let cover = rig
        .tree_mut()
        .add_root(UiLayer::GamePopup, at(0.0, 0.0, 200.0, 100.0));
    rig.tree_mut().set_interactive(cover, true);
    let at_a = rig.centre(a);
    let events = rig.click_px(at_a);
    assert_eq!(
        activations(&events),
        [a],
        "the routing before its first layout"
    );
    let events = rig.click_px(at_a);
    assert_eq!(activations(&events), [cover], "laid out at the frame's end");
}

#[test]
fn hidden_game_layers_take_no_input() {
    let (mut rig, _root, a, _b) = Rig::new();
    rig.world.insert_resource(UiDebug {
        hide_game_layers: true,
        show_bounds: false,
    });
    let events = rig.click(a);
    assert!(events.is_empty(), "{events:?}");
    assert!(rig.routed().mouse_just_pressed(LEFT), "the game's click");
    assert!(!rig.router().pointer_over_ui());
}

#[test]
fn a_world_without_the_queue_the_router_or_the_tree_still_feeds_the_routed_view() {
    let mut world = World::new();
    world.insert_resource(InputState::new());
    world.insert_resource(RoutedInput::default());
    world.resource_mut::<InputState>().key_down(KeyCode::Escape);
    ui_route_system(&mut world);
    assert!(
        world
            .resource::<RoutedInput>()
            .just_pressed(KeyCode::Escape)
    );
    assert!(world.has_resource::<UiRouter>(), "the router is inserted");
    assert!(!world.has_resource::<EventQueue<UiEvent>>());
    world.resource_mut::<InputState>().begin_frame();
    world.resource_mut::<InputState>().key_up(KeyCode::Escape);
    ui_route_system(&mut world);
    assert!(
        world
            .resource::<RoutedInput>()
            .just_released(KeyCode::Escape)
    );
}

#[test]
fn a_widget_under_a_disabled_container_takes_no_click_or_press() {
    // `D-125`: disabling a container disables what is inside it.
    let (mut rig, root, _a, _b) = Rig::new();
    let panel = rig.tree_mut().panel(root, at(150.0, 10.0, 200.0, 100.0));
    let inner = rig.place(panel, at(0.0, 0.0, 100.0, 30.0));
    rig.layout();
    rig.tree_mut().set_enabled(panel, false);
    let events = rig.click(inner);
    assert!(activations(&events).is_empty(), "{events:?}");
    assert_eq!(rig.tree().pressed(), None);

    // A press held when its container is disabled ends cancelled.
    rig.tree_mut().set_enabled(panel, true);
    let point = rig.centre(inner);
    rig.frame(|input| {
        input.update_cursor_position(point.x, point.y);
        input.mouse_down(LEFT);
    });
    assert_eq!(rig.tree().pressed(), Some(inner));
    rig.tree_mut().set_enabled(panel, false);
    let events = rig.frame(|input| input.mouse_up(LEFT));
    assert!(
        events.contains(&UiEvent::PressCancelled { widget: inner }),
        "{events:?}"
    );
    assert!(activations(&events).is_empty());

    rig.tree_mut().set_enabled(panel, true);
    assert_eq!(activations(&rig.click(inner)), [inner]);
}

// ---- gamepads (M54) ----

const PAD: GamepadId = GamepadId::new(0);
const PAD_B: GamepadId = GamepadId::new(1);
const STICK_X: GamepadAxis = GamepadAxis::LeftStickX;

#[test]
fn dpad_moves_focus_and_south_activates() {
    let (mut rig, _root, a, b) = Rig::new();
    rig.tree_mut().focus(a);
    rig.frame(|input| input.gamepad_button_down(PAD, GamepadButton::DPadDown));
    assert_eq!(rig.tree().focused(), Some(b));
    assert_eq!(rig.router().last_device(), InputDevice::Gamepad);
    assert!(
        !rig.routed().gamepad_just_pressed(GamepadButton::DPadDown),
        "the UI took the press"
    );
    let events = rig.frame(|input| {
        input.gamepad_button_up(PAD, GamepadButton::DPadDown);
        input.gamepad_button_down(PAD, GamepadButton::South);
    });
    assert_eq!(activations(&events), [b]);
    assert!(
        !rig.routed().gamepad_just_released(GamepadButton::DPadDown)
            && !rig.routed().gamepad_just_pressed(GamepadButton::South),
        "and its release, and South"
    );
    assert!(!rig.game_pressed(UI_ACCEPT));
    let events = rig.frame(|input| input.gamepad_button_up(PAD, GamepadButton::South));
    assert!(activations(&events).is_empty());
    assert!(!rig.routed().gamepad_just_released(GamepadButton::South));

    // East cancels the focused scope, as Escape does.
    let events = rig.frame(|input| input.gamepad_button_down(PAD, GamepadButton::East));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, UiEvent::Cancel { .. }))
    );
}

#[test]
fn a_held_stick_repeats_navigation() {
    let (mut rig, root) = Rig::empty();
    let column: Vec<WidgetId> = (0..12)
        .map(|i| rig.place(root, at(10.0, 24.0 * i as f32, 100.0, 20.0)))
        .collect();
    rig.layout();
    let index = |rig: &Rig| {
        column
            .iter()
            .position(|id| rig.tree().focused() == Some(*id))
            .expect("focus in the column")
    };
    rig.tree_mut().focus(column[0]);
    // Down on the left stick: y grows upward.
    rig.frame(|input| input.set_gamepad_axis(PAD, GamepadAxis::LeftStickY, -0.9));
    assert_eq!(index(&rig), 1, "the press moves once");

    let mut moved = Vec::new();
    for frame in 1..=26 {
        let before = rig.tree().focused();
        // Small moves inside the held range do not restart it.
        rig.frame(|input| {
            input.set_gamepad_axis(
                PAD,
                GamepadAxis::LeftStickY,
                -0.9 + 0.01 * (frame % 2) as f32,
            );
        });
        if rig.tree().focused() != before {
            moved.push(frame);
        }
    }
    assert_eq!(moved, [16, 20, 23, 26]);
    assert_eq!(index(&rig), 5);

    rig.frame(|input| input.set_gamepad_axis(PAD, GamepadAxis::LeftStickY, -0.2));
    for _ in 0..40 {
        rig.frame(|_| {});
    }
    assert_eq!(index(&rig), 5, "the release stops it");

    // The d-pad repeats too.
    rig.frame(|input| input.gamepad_button_down(PAD, GamepadButton::DPadUp));
    assert_eq!(index(&rig), 4);
    for _ in 1..=16 {
        rig.frame(|_| {});
    }
    assert_eq!(index(&rig), 3, "after half a second");
}

#[test]
fn a_stick_the_ui_took_reads_zero_when_routed() {
    let (mut rig, _root, a, _b) = Rig::new();
    rig.tree_mut().focus(a);
    let mut routed = Vec::new();
    for value in [0.2, 0.4, 0.6, 0.8, 0.3, 0.1] {
        rig.frame(|input| input.set_gamepad_axis(PAD, STICK_X, value));
        let view = rig.routed();
        routed.push(view.gamepad_axis(STICK_X));
        for direction in [AxisDirection::Negative, AxisDirection::Positive] {
            assert!(
                !view.is_stick_pressed(STICK_X, direction)
                    && !view.stick_just_pressed(STICK_X, direction)
                    && !view.stick_just_released(STICK_X, direction),
                "no stick edge at {value}"
            );
        }
    }
    assert_eq!(routed, [0.2, 0.4, 0.0, 0.0, 0.3, 0.1]);
    let input = rig.world.resource::<InputState>();
    assert_eq!(
        input.gamepad_axis(STICK_X),
        0.1,
        "the raw view reads it all"
    );
}

#[test]
fn gamepad_navigation_shows_focus() {
    let (mut rig, _root, a, b) = Rig::new();
    rig.click(a);
    assert!(!rig.tree().focus_visible());
    // The first press shows focus and goes to the game, as a key's does.
    rig.frame(|input| input.gamepad_button_down(PAD, GamepadButton::DPadDown));
    assert_eq!(rig.router().last_device(), InputDevice::Gamepad);
    assert!(rig.tree().focus_visible());
    assert!(rig.router().wants_keyboard());
    assert!(rig.routed().gamepad_just_pressed(GamepadButton::DPadDown));
    assert_eq!(rig.tree().focused(), Some(a));
    rig.frame(|input| input.gamepad_button_up(PAD, GamepadButton::DPadDown));
    rig.frame(|input| input.gamepad_button_down(PAD, GamepadButton::DPadDown));
    assert_eq!(rig.tree().focused(), Some(b), "the next one navigates");

    // A stick press shows it too.
    rig.click(a);
    assert!(!rig.tree().focus_visible());
    rig.frame(|input| input.set_gamepad_axis(PAD, STICK_X, 0.7));
    assert!(rig.tree().focus_visible());
}

#[test]
fn two_pads_keep_their_own_presses() {
    let (mut rig, _root, _a, _b) = Rig::new();
    // Nothing focused: pad A's South is the game's.
    rig.frame(|input| input.gamepad_button_down(PAD, GamepadButton::South));
    assert!(
        rig.routed()
            .is_gamepad_pressed_on(PAD, GamepadButton::South)
    );

    let popup = rig
        .tree_mut()
        .add_root(UiLayer::GamePopup, at(150.0, 100.0, 200.0, 100.0));
    rig.tree_mut().set_modal(popup, true);
    let ok = rig.place(popup, at(10.0, 10.0, 80.0, 30.0));
    rig.layout();
    let events = rig.frame(|input| input.gamepad_button_down(PAD_B, GamepadButton::South));
    assert_eq!(activations(&events), [ok], "pad B's South is the modal's");
    assert!(
        !rig.routed()
            .is_gamepad_pressed_on(PAD_B, GamepadButton::South)
    );

    rig.frame(|input| input.gamepad_button_up(PAD, GamepadButton::South));
    let routed = rig.routed();
    assert!(
        routed.gamepad_just_released(GamepadButton::South)
            && !routed.is_gamepad_pressed_on(PAD, GamepadButton::South),
        "pad A's release reaches the game"
    );
    rig.frame(|input| input.gamepad_button_up(PAD_B, GamepadButton::South));
    assert!(
        !rig.routed().gamepad_just_released(GamepadButton::South),
        "pad B's is the UI's"
    );

    // A disconnect forgets the pad's UI presses.
    rig.frame(|input| input.gamepad_button_down(PAD_B, GamepadButton::DPadUp));
    rig.frame(|input| input.gamepad_disconnected(PAD_B));
    assert!(rig.router().ui_held.is_empty());
    assert!(rig.router().repeat.is_none());
    assert_eq!(rig.routed().gamepads().collect::<Vec<_>>(), [PAD]);
}

#[test]
fn same_frame_crossings_reach_the_routed_view() {
    // No focusable widget: the UI takes nothing.
    let (mut rig, _root) = Rig::empty();
    let views = |rig: &Rig| {
        let read = |input: &InputState| {
            (
                input.is_stick_pressed(STICK_X, AxisDirection::Positive),
                input.stick_just_pressed(STICK_X, AxisDirection::Positive),
                input.stick_just_released(STICK_X, AxisDirection::Positive),
                input.gamepad_axis(STICK_X),
            )
        };
        (read(rig.world.resource::<InputState>()), read(rig.routed()))
    };
    rig.frame(|input| input.set_gamepad_axis(PAD, STICK_X, 0.8));
    let (raw, routed) = views(&rig);
    assert_eq!(raw, (true, true, false, 0.8));
    assert_eq!(routed, raw);

    rig.frame(|input| {
        input.set_gamepad_axis(PAD, STICK_X, 0.3);
        input.set_gamepad_axis(PAD, STICK_X, 0.4);
    });
    let (raw, routed) = views(&rig);
    assert_eq!(
        raw,
        (false, false, true, 0.4),
        "released and not pressed again"
    );
    assert_eq!(routed, raw);

    rig.frame(|input| {
        input.set_gamepad_axis(PAD, STICK_X, 0.2);
        input.set_gamepad_axis(PAD, STICK_X, 0.6);
        input.set_gamepad_axis(PAD, STICK_X, 0.45);
    });
    let (raw, routed) = views(&rig);
    assert_eq!(raw, (true, true, false, 0.45), "pressed and still held");
    assert_eq!(routed, raw);
}

/// The `ValueChanged` events, in order.
fn values(events: &[UiEvent]) -> Vec<(WidgetId, UiValue)> {
    events
        .iter()
        .filter_map(|event| match event {
            UiEvent::ValueChanged { widget, value } => Some((*widget, *value)),
            _ => None,
        })
        .collect()
}

/// `min + k·step` as a range computes it: in `f64`, stored as `f32`.
fn allowed(min: f32, step: f32, k: u32) -> f32 {
    (f64::from(min) + f64::from(k) * f64::from(step)) as f32
}

impl Rig {
    /// The root with a slider `s` at (10, 10), 200 by 20, over `range` at
    /// `value`, and button `b` below it at (10, 60): the thumb is 20 wide,
    /// so its centre runs from x 20 to 200.
    fn slider(range: SliderRange, value: f32) -> (Self, WidgetId, WidgetId) {
        let (mut rig, root) = Self::empty();
        let tree = rig.tree_mut();
        let slider = tree.slider(root, range, value);
        tree.set_layout_style(slider, at(10.0, 10.0, 200.0, 20.0));
        let b = rig.place(root, at(10.0, 60.0, 100.0, 30.0));
        rig.layout();
        (rig, slider, b)
    }

    /// The x where `fraction` of the slider's travel lies.
    fn along(fraction: f32) -> f32 {
        20.0 + 180.0 * fraction
    }

    fn value(&self, slider: WidgetId) -> f32 {
        self.tree().slider_value(slider).expect("a slider")
    }

    /// A press of `key`, then its release in the next frame; the press's
    /// events.
    fn tap(&mut self, key: KeyCode) -> Vec<UiEvent> {
        let events = self.frame(|input| input.key_down(key));
        self.frame(|input| input.key_up(key));
        events
    }
}

#[test]
fn a_click_and_an_accept_press_flip_a_toggle_after_its_activation() {
    let (mut rig, root) = Rig::empty();
    let toggle = rig.tree_mut().toggle(root, false);
    rig.tree_mut()
        .set_layout_style(toggle, at(10.0, 10.0, 20.0, 20.0));
    rig.layout();
    let events = rig.click(toggle);
    assert_eq!(
        events,
        [
            UiEvent::PointerEntered { widget: toggle },
            UiEvent::Pressed {
                widget: toggle,
                button: LEFT
            },
            UiEvent::FocusGained { widget: toggle },
            UiEvent::Released {
                widget: toggle,
                button: LEFT
            },
            UiEvent::Activated { widget: toggle },
            UiEvent::ValueChanged {
                widget: toggle,
                value: UiValue::Bool(true)
            },
        ]
    );
    assert_eq!(rig.tree().toggled(toggle), Some(true));
    assert_eq!(rig.queue().value_changed(toggle), Some(UiValue::Bool(true)));

    // A second click soon after: a double click, which flips it back.
    let events = rig.click(toggle);
    assert_eq!(
        events[2..],
        [
            UiEvent::Activated { widget: toggle },
            UiEvent::ValueChanged {
                widget: toggle,
                value: UiValue::Bool(false)
            },
            UiEvent::DoubleClicked { widget: toggle },
        ]
    );
    assert_eq!(rig.tree().toggled(toggle), Some(false));

    rig.frame(|input| input.key_down(KeyCode::KeyW));
    assert!(rig.tree().focus_visible(), "a key shows focus");
    let events = rig.frame(|input| input.key_down(KeyCode::Enter));
    assert_eq!(
        events,
        [
            UiEvent::Activated { widget: toggle },
            UiEvent::ValueChanged {
                widget: toggle,
                value: UiValue::Bool(true)
            },
        ]
    );
    assert_eq!(rig.tree().toggled(toggle), Some(true));
}

#[test]
fn a_press_sets_a_slider_from_the_pointer_and_moves_follow_past_both_ends() {
    let range = SliderRange::new(0.0, 10.0).with_step(1.0);
    let (mut rig, slider, _b) = Rig::slider(range, 5.0);
    let x = Rig::along(0.3);
    let events = rig.frame(|input| {
        input.update_cursor_position(x, 20.0);
        input.mouse_down(LEFT);
    });
    assert_eq!(
        events,
        [
            UiEvent::PointerEntered { widget: slider },
            UiEvent::Pressed {
                widget: slider,
                button: LEFT
            },
            UiEvent::FocusGained { widget: slider },
            UiEvent::ValueChanged {
                widget: slider,
                value: UiValue::Number(3.0)
            },
        ],
        "the value after the press's own events"
    );

    let events = rig.frame(|input| input.update_cursor_position(300.0, 200.0));
    assert_eq!(
        values(&events),
        [(slider, UiValue::Number(10.0))],
        "past the end and off the slider"
    );
    let events = rig.frame(|input| input.update_cursor_position(-50.0, 20.0));
    assert_eq!(values(&events), [(slider, UiValue::Number(0.0))]);
    let events = rig.frame(|input| input.update_cursor_position(Rig::along(0.04), 20.0));
    assert!(
        values(&events).is_empty(),
        "a move that snaps to the same value sends nothing"
    );
    let events = rig.frame(|input| input.update_cursor_position(Rig::along(0.81), 20.0));
    assert_eq!(values(&events), [(slider, UiValue::Number(8.0))]);
    assert_eq!(
        rig.queue().value_changed(slider),
        Some(UiValue::Number(8.0))
    );

    let events = rig.frame(|input| {
        input.update_cursor_position(Rig::along(0.2), 20.0);
        input.mouse_up(LEFT);
    });
    assert_eq!(
        values(&events),
        [(slider, UiValue::Number(2.0))],
        "the move before the release"
    );
    assert_eq!(activations(&events), [slider], "a click still activates");
    let events = rig.frame(|input| input.update_cursor_position(Rig::along(0.9), 20.0));
    assert!(values(&events).is_empty(), "the release ended the drag");
    assert_eq!(rig.value(slider), 2.0);

    let events = rig.frame(|input| input.mouse_down(MouseButton::Right));
    assert!(events.contains(&UiEvent::Pressed {
        widget: slider,
        button: MouseButton::Right
    }));
    let events = rig.frame(|input| input.update_cursor_position(Rig::along(0.5), 20.0));
    assert!(values(&events).is_empty(), "another button changes nothing");
    assert_eq!(rig.value(slider), 2.0);
}

#[test]
fn arrows_step_a_focused_slider_with_the_repeat_and_stop_at_the_ends() {
    // 0 to 1 by 0.3: 0, 0.3, 0.6, 0.9 and 1.
    let range = SliderRange::new(0.0, 1.0).with_step(0.3);
    let (mut rig, slider, b) = Rig::slider(range, 0.0);
    rig.tree_mut().focus(slider);
    let events = rig.frame(|input| input.key_down(KeyCode::ArrowRight));
    assert_eq!(
        values(&events),
        [(slider, UiValue::Number(allowed(0.0, 0.3, 1)))]
    );
    assert_eq!(
        rig.tree().focused(),
        Some(slider),
        "the arrow stepped, not moved"
    );
    assert!(!rig.routed().is_pressed(KeyCode::ArrowRight));

    // The navigation repeat: frames 16, 20, 23 and 26 of 1/32 s; the last
    // finds the slider at its end.
    let mut changed = Vec::new();
    for frame in 1..=26 {
        if !values(&rig.frame(|_| {})).is_empty() {
            changed.push(frame);
        }
    }
    assert_eq!(changed, [16, 20, 23]);
    assert_eq!(rig.value(slider), 1.0, "max is reachable");
    rig.frame(|input| input.key_up(KeyCode::ArrowRight));
    let events = rig.tap(KeyCode::ArrowRight);
    assert!(values(&events).is_empty(), "at the end: nothing");
    assert!(
        !rig.routed().just_released(KeyCode::ArrowRight),
        "and the press stays the UI's"
    );

    let events = rig.tap(KeyCode::ArrowLeft);
    assert_eq!(
        values(&events),
        [(slider, UiValue::Number(allowed(0.0, 0.3, 3)))],
        "back to the last whole step"
    );
    for _ in 0..3 {
        rig.tap(KeyCode::ArrowLeft);
    }
    assert_eq!(rig.value(slider), 0.0);
    assert!(values(&rig.tap(KeyCode::ArrowLeft)).is_empty());

    rig.tap(KeyCode::ArrowDown);
    assert_eq!(
        rig.tree().focused(),
        Some(b),
        "up and down still move focus"
    );
    rig.tap(KeyCode::ArrowUp);
    assert_eq!(rig.tree().focused(), Some(slider));
}

#[test]
fn a_step_finer_than_the_f32_spacing_still_moves_each_press() {
    // At 2^24 the f32 spacing is 2, so min + 0.5 stores as min.
    let range = SliderRange::new(16_777_216.0, 16_777_220.0).with_step(0.5);
    let (mut rig, slider, _b) = Rig::slider(range, 16_777_216.0);
    rig.tree_mut().focus(slider);
    let events = rig.tap(KeyCode::ArrowRight);
    assert_eq!(values(&events), [(slider, UiValue::Number(16_777_218.0))]);
    let events = rig.tap(KeyCode::ArrowRight);
    assert_eq!(values(&events), [(slider, UiValue::Number(16_777_220.0))]);
    assert!(values(&rig.tap(KeyCode::ArrowRight)).is_empty());
    let events = rig.tap(KeyCode::ArrowLeft);
    assert_eq!(values(&events), [(slider, UiValue::Number(16_777_218.0))]);

    // A continuous range moves a tenth of its span.
    let (mut rig, slider, _b) = Rig::slider(SliderRange::new(-1.0, 1.0), 0.0);
    rig.tree_mut().focus(slider);
    rig.tap(KeyCode::ArrowRight);
    assert_eq!(rig.value(slider), 0.2);
}

#[test]
fn the_first_arrow_after_a_pointer_press_is_the_games_and_the_next_steps() {
    let range = SliderRange::new(0.0, 10.0).with_step(1.0);
    let (mut rig, slider, _b) = Rig::slider(range, 5.0);
    rig.click_px(Vec2::new(Rig::along(0.0), 20.0));
    assert_eq!(rig.value(slider), 0.0);
    assert!(!rig.tree().focus_visible());
    let events = rig.frame(|input| input.key_down(KeyCode::ArrowRight));
    assert!(
        values(&events).is_empty(),
        "hidden focus: the game's (D-149)"
    );
    assert!(rig.routed().just_pressed(KeyCode::ArrowRight));
    rig.frame(|input| input.key_up(KeyCode::ArrowRight));
    let events = rig.tap(KeyCode::ArrowRight);
    assert_eq!(values(&events), [(slider, UiValue::Number(1.0))]);
}

#[test]
fn a_singleton_slider_takes_a_press_and_arrows_and_changes_nothing() {
    let (mut rig, slider, _b) = Rig::slider(SliderRange::new(3.0, 3.0), 3.0);
    let events = rig.click_px(Vec2::new(Rig::along(0.8), 20.0));
    assert!(activations(&events).contains(&slider), "the UI's press");
    assert!(values(&events).is_empty());
    rig.tap(KeyCode::KeyW);
    let events = rig.tap(KeyCode::ArrowRight);
    assert!(values(&events).is_empty());
    assert!(
        !rig.routed().just_released(KeyCode::ArrowRight),
        "the UI's key"
    );
    assert_eq!(rig.value(slider), 3.0);
}

#[test]
fn a_disabled_or_hidden_control_takes_nothing() {
    let range = SliderRange::new(0.0, 10.0).with_step(1.0);
    let (mut rig, slider, _b) = Rig::slider(range, 5.0);
    let root = rig.tree().root_of(slider).expect("in the tree");
    let toggle = rig.tree_mut().toggle(root, false);
    rig.tree_mut()
        .set_layout_style(toggle, at(250.0, 10.0, 20.0, 20.0));
    rig.layout();
    rig.tree_mut().set_enabled(toggle, false);
    rig.tree_mut().set_visibility(slider, Visibility::Hidden);
    let events = rig.click(toggle);
    assert!(events.is_empty(), "{events:?}");
    let events = rig.click_px(Vec2::new(Rig::along(0.9), 20.0));
    assert!(events.is_empty(), "{events:?}");
    assert_eq!(rig.tree().toggled(toggle), Some(false));
    assert_eq!(rig.value(slider), 5.0);
}

#[test]
fn a_press_cancelled_by_focus_loss_keeps_the_value() {
    let range = SliderRange::new(0.0, 10.0).with_step(1.0);
    let (mut rig, slider, _b) = Rig::slider(range, 5.0);
    rig.frame(|input| {
        input.update_cursor_position(Rig::along(0.7), 20.0);
        input.mouse_down(LEFT);
    });
    assert_eq!(rig.value(slider), 7.0);
    let events = rig.frame(|input| input.set_focused(false));
    assert!(events.contains(&UiEvent::PressCancelled { widget: slider }));
    assert!(values(&events).is_empty());
    assert_eq!(rig.value(slider), 7.0);
    rig.frame(|input| input.set_focused(true));
    let events = rig.frame(|input| {
        input.update_cursor_position(Rig::along(0.1), 20.0);
        input.mouse_up(LEFT);
    });
    assert!(
        values(&events).is_empty(),
        "the cancelled press drags nothing"
    );
    assert_eq!(rig.value(slider), 7.0);
}

#[test]
fn the_setters_route_no_value_change() {
    let range = SliderRange::new(0.0, 10.0).with_step(1.0);
    let (mut rig, slider, _b) = Rig::slider(range, 5.0);
    let root = rig.tree().root_of(slider).expect("in the tree");
    let toggle = rig.tree_mut().toggle(root, false);
    assert!(rig.tree_mut().set_slider_value(slider, 9.0));
    assert!(rig.tree_mut().set_toggled(toggle, true));
    rig.tree_mut()
        .set_slider_range(slider, SliderRange::new(0.0, 4.0));
    let events = rig.frame(|_| {});
    assert!(values(&events).is_empty(), "{events:?}");
    assert_eq!(rig.value(slider), 4.0);
}

// ---- M56: frozen roots and transforms ----

#[test]
fn a_frozen_modal_root_keeps_the_keyboard_and_blocks_the_pointer_while_nothing_in_it_acts() {
    let (mut rig, _root, a, _b) = Rig::new();
    let popup = rig
        .tree_mut()
        .add_root(UiLayer::GamePopup, at(100.0, 100.0, 200.0, 100.0));
    rig.tree_mut().set_modal(popup, true);
    let ok = rig.place(popup, at(10.0, 10.0, 80.0, 30.0));
    rig.layout();
    rig.frame(|_| {});
    assert_eq!(rig.tree().focused(), Some(ok));

    // A press held over `ok` when the root freezes is cancelled.
    let at_ok = rig.centre(ok);
    rig.frame(|input| {
        input.update_cursor_position(at_ok.x, at_ok.y);
        input.mouse_down(LEFT);
    });
    assert_eq!(rig.tree().pressed(), Some(ok));
    rig.tree_mut().set_frozen(popup, true);
    let events = rig.frame(|input| input.mouse_up(LEFT));
    assert!(events.contains(&UiEvent::PressCancelled { widget: ok }));
    assert!(activations(&events).is_empty());
    assert!(
        !rig.routed().mouse_just_released(LEFT),
        "the release is the UI's"
    );

    for key in [
        KeyCode::Enter,
        KeyCode::Space,
        KeyCode::ArrowDown,
        KeyCode::Tab,
    ] {
        let events = rig.tap(key);
        assert!(activations(&events).is_empty(), "{key:?} activates nothing");
        assert!(!rig.routed().just_pressed(key), "{key:?} is the UI's");
        assert_eq!(rig.tree().focused(), Some(ok), "{key:?} moves no focus");
    }
    let events = rig.frame(|input| input.key_down(KeyCode::Escape));
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, UiEvent::Cancel { .. })),
        "Escape sends no Cancel"
    );
    assert!(!rig.routed().just_pressed(KeyCode::Escape));
    assert!(!rig.game_pressed("engine_exit"));
    assert!(rig.router().wants_keyboard());
    rig.frame(|input| input.key_up(KeyCode::Escape));

    let events = rig.click(ok);
    assert!(
        events.is_empty(),
        "no press, hover or activation: {events:?}"
    );
    let events = rig.click(a);
    assert!(events.is_empty(), "the modal blocks what is under it");
    assert!(!rig.routed().mouse_just_pressed(LEFT));
    assert!(rig.router().pointer_over_ui());

    rig.tree_mut().set_frozen(popup, false);
    let events = rig.tap(KeyCode::Enter);
    assert_eq!(activations(&events), [ok], "thawed, it acts again");
}

#[test]
fn a_translated_slider_maps_the_pointer_where_it_paints() {
    let range = SliderRange::new(0.0, 100.0);
    let (mut rig, slider, _b) = Rig::slider(range, 0.0);
    let x = Rig::along(0.5);
    rig.click_px(Vec2::new(x, 20.0));
    let untouched = rig.value(slider);
    rig.tree_mut().set_slider_value(slider, 0.0);
    // 20.4 rounds to 20 whole pixels at the last paint's scale, 1.
    rig.tree_mut().set_translation(slider, Vec2::new(20.4, 0.0));
    assert_eq!(
        rig.tree().visual_rect(slider),
        Some(Rect::from_pos_size(
            Vec2::new(30.0, 10.0),
            Vec2::new(200.0, 20.0)
        ))
    );
    rig.click_px(Vec2::new(x + 20.0, 20.0));
    assert_eq!(rig.value(slider), untouched);
    assert_eq!(
        rig.tree().hit_test(Vec2::new(225.0, 20.0)),
        Some(slider),
        "past the layout box, inside the painted one"
    );
    assert_ne!(rig.tree().hit_test(Vec2::new(15.0, 20.0)), Some(slider));
}

// ---- M56: the capture-next-input mode ----

fn captured(events: &[UiEvent]) -> Vec<(WidgetId, Binding)> {
    events
        .iter()
        .filter_map(|event| match *event {
            UiEvent::InputCaptured { widget, binding } => Some((widget, binding)),
            _ => None,
        })
        .collect()
}

fn cancelled(events: &[UiEvent]) -> Vec<WidgetId> {
    events
        .iter()
        .filter_map(|event| match *event {
            UiEvent::CaptureCancelled { widget } => Some(widget),
            _ => None,
        })
        .collect()
}

fn start_capture(rig: &mut Rig, widget: WidgetId) {
    rig.world
        .resource_mut::<UiRouter>()
        .capture_next_input(widget);
}

#[test]
fn a_capture_takes_one_key_mouse_pad_or_stick_press_and_withholds_its_release() {
    let (mut rig, _root, a, _b) = Rig::new();

    start_capture(&mut rig, a);
    rig.frame(|_| {});
    assert!(
        rig.router().wants_keyboard(),
        "a pending capture has the keyboard"
    );
    let events = rig.frame(|input| input.key_down(KeyCode::KeyK));
    assert_eq!(
        captured(&events),
        [(
            a,
            Binding::Key {
                code: KeyCode::KeyK
            }
        )]
    );
    assert_eq!(rig.router().capturing(), None);
    assert!(!rig.routed().just_pressed(KeyCode::KeyK));
    rig.frame(|input| input.key_up(KeyCode::KeyK));
    assert!(
        !rig.routed().just_released(KeyCode::KeyK),
        "the release too"
    );

    start_capture(&mut rig, a);
    let events = rig.frame(|input| input.mouse_down(MouseButton::Right));
    assert_eq!(
        captured(&events),
        [(
            a,
            Binding::Mouse {
                button: MouseButton::Right
            }
        )]
    );
    assert!(!rig.routed().mouse_just_pressed(MouseButton::Right));
    rig.frame(|input| input.mouse_up(MouseButton::Right));
    assert!(!rig.routed().mouse_just_released(MouseButton::Right));

    start_capture(&mut rig, a);
    let events = rig.frame(|input| input.gamepad_button_down(PAD, GamepadButton::North));
    assert_eq!(
        captured(&events),
        [(
            a,
            Binding::GamepadButton {
                button: GamepadButton::North
            }
        )]
    );
    assert!(!rig.routed().gamepad_just_pressed(GamepadButton::North));
    rig.frame(|input| input.gamepad_button_up(PAD, GamepadButton::North));
    assert!(!rig.routed().gamepad_just_released(GamepadButton::North));

    start_capture(&mut rig, a);
    let events = rig.frame(|input| input.set_gamepad_axis(PAD, STICK_X, 0.9));
    assert_eq!(
        captured(&events),
        [(
            a,
            Binding::GamepadAxis {
                axis: STICK_X,
                direction: AxisDirection::Positive
            }
        )]
    );
    assert_eq!(rig.routed().gamepad_axis(STICK_X), 0.0, "withheld");
    rig.frame(|input| input.set_gamepad_axis(PAD, STICK_X, 0.0));
    assert!(
        !rig.routed()
            .stick_just_pressed(STICK_X, AxisDirection::Positive)
    );
    assert_eq!(rig.router().last_device(), InputDevice::Gamepad);
}

#[test]
fn escape_is_captured_a_repeat_is_not_and_the_wheel_is_taken() {
    let (mut rig, _root, a, _b) = Rig::new();
    // W held by the game before the capture starts.
    rig.frame(|input| input.key_down(KeyCode::KeyW));
    start_capture(&mut rig, a);
    let events = rig.frame(|input| {
        input.key_down(KeyCode::KeyW);
        input.add_scroll_line_delta(0.0, 1.0);
    });
    assert!(captured(&events).is_empty(), "an OS repeat and the wheel");
    assert_eq!(rig.router().capturing(), Some(a));
    assert_eq!(
        rig.routed().scroll_line_delta(),
        (0.0, 0.0),
        "the wheel is taken"
    );
    let events = rig.frame(|input| input.key_down(KeyCode::Escape));
    assert_eq!(
        captured(&events),
        [(
            a,
            Binding::Key {
                code: KeyCode::Escape
            }
        )]
    );
    assert!(events.iter().all(|e| !matches!(e, UiEvent::Cancel { .. })));
    assert!(!rig.game_pressed("engine_exit"));
    rig.frame(|input| input.key_up(KeyCode::Escape));
    assert!(!rig.routed().just_released(KeyCode::Escape));
}

#[test]
fn nothing_navigates_while_a_capture_is_pending() {
    let (mut rig, _root, a, b) = Rig::new();
    rig.tree_mut().focus(a);
    rig.frame(|_| {});
    // A held direction's repeat stops too.
    rig.frame(|input| input.key_down(KeyCode::ArrowDown));
    assert_eq!(rig.tree().focused(), Some(b));
    rig.tree_mut().focus(a);
    start_capture(&mut rig, a);
    for _ in 0..40 {
        rig.frame(|_| {});
    }
    assert_eq!(rig.tree().focused(), Some(a), "no repeat moved it");
    rig.frame(|input| input.key_up(KeyCode::ArrowDown));
    let events = rig.frame(|input| input.key_down(KeyCode::Tab));
    assert_eq!(captured(&events).len(), 1);
    assert_eq!(rig.tree().focused(), Some(a));
    assert!(activations(&events).is_empty());
}

#[test]
fn a_capture_ends_without_input_for_each_cancelling_cause() {
    // Focus loss.
    let (mut rig, _root, a, _b) = Rig::new();
    start_capture(&mut rig, a);
    let events = rig.frame(|input| input.set_focused(false));
    assert_eq!(cancelled(&events), [a]);
    assert_eq!(rig.router().capturing(), None);

    // Removal, hiding, disabling and freezing.
    type Cause = fn(&mut UiTree, WidgetId, WidgetId);
    let causes: [Cause; 4] = [
        |tree, _root, a| tree.remove(a),
        |tree, _root, a| tree.set_visibility(a, Visibility::Hidden),
        |tree, _root, a| tree.set_enabled(a, false),
        |tree, root, _a| tree.set_frozen(root, true),
    ];
    for cause in causes {
        let (mut rig, root, a, _b) = Rig::new();
        start_capture(&mut rig, a);
        cause(rig.tree_mut(), root, a);
        let events = rig.frame(|input| input.key_down(KeyCode::KeyK));
        assert_eq!(cancelled(&events), [a]);
        assert!(captured(&events).is_empty());
    }

    // `cancel_capture` and a second capture.
    let (mut rig, _root, a, b) = Rig::new();
    start_capture(&mut rig, a);
    rig.world.resource_mut::<UiRouter>().cancel_capture();
    let events = rig.frame(|_| {});
    assert_eq!(cancelled(&events), [a]);
    start_capture(&mut rig, a);
    start_capture(&mut rig, b);
    let events = rig.frame(|input| input.key_down(KeyCode::KeyK));
    assert_eq!(cancelled(&events), [a]);
    assert_eq!(
        captured(&events),
        [(
            b,
            Binding::Key {
                code: KeyCode::KeyK
            }
        )]
    );
}

// ---- M58: raising a window on a press ----

#[test]
fn a_primary_press_raises_a_window_root_but_not_another_role_or_a_frozen_one() {
    let (mut rig, game) = Rig::empty();
    let tree = rig.tree_mut();
    let a = tree.add_root(UiLayer::Debug, at(0.0, 0.0, 200.0, 200.0));
    let b = tree.add_root(UiLayer::Debug, at(100.0, 100.0, 200.0, 150.0));
    tree.set_role(a, Role::Window);
    tree.set_role(b, Role::Window);
    let ok = tree.button(a, "ok");
    tree.set_layout_style(ok, at(10.0, 150.0, 50.0, 30.0));
    rig.layout();

    let events = rig.click_px(Vec2::new(30.0, 165.0));
    assert_eq!(rig.tree().roots(), [game, b, a], "a rises over b");
    assert_eq!(activations(&events), [ok], "and takes the press");
    rig.click_px(Vec2::new(150.0, 150.0));
    assert_eq!(rig.tree().roots(), [game, b, a], "a press on the front one");
    rig.click_px(Vec2::new(250.0, 220.0));
    assert_eq!(rig.tree().roots(), [game, a, b]);
    rig.frame(|input| {
        input.update_cursor_position(50.0, 50.0);
        input.mouse_down(MouseButton::Right);
        input.mouse_up(MouseButton::Right);
    });
    assert_eq!(rig.tree().roots(), [game, a, b], "a primary press only");

    rig.tree_mut().set_frozen(a, true);
    rig.click_px(Vec2::new(50.0, 50.0));
    assert_eq!(rig.tree().roots(), [game, a, b], "a frozen window stays");
    rig.tree_mut().set_frozen(a, false);
    rig.tree_mut().set_role(a, Role::None);
    rig.click_px(Vec2::new(50.0, 50.0));
    assert_eq!(
        rig.tree().roots(),
        [game, a, b],
        "a panel's own role is not a window's"
    );
    rig.click_px(Vec2::new(350.0, 30.0));
    assert_eq!(rig.tree().roots(), [game, a, b], "a miss changes nothing");
}
