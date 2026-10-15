use std::sync::atomic::{AtomicU32, Ordering};

use super::*;
use crate::display::ScaleFactor;
use crate::ecs::{EventQueue, World};
use crate::input::{ActionMap, InputState, MouseButton, RoutedInput};
use crate::time::Time;
use crate::ui::{
    CursorIcon, FixedAdvanceMeasure, LayoutStyle, Sizing, UiEvent, UiLayer, UiRouter,
    ui_route_system,
};

const VIEWPORT: Vec2 = Vec2::new(400.0, 300.0);
const DT: f32 = 1.0 / 32.0;
const LEFT: MouseButton = MouseButton::Left;

fn fixed(w: f32, h: f32) -> LayoutStyle {
    LayoutStyle::default().with_size(Sizing::Px(w), Sizing::Px(h))
}

fn at(x: f32, y: f32, w: f32, h: f32) -> LayoutStyle {
    fixed(w, h).anchored(Anchor::TopStart, Vec2::new(x, y))
}

fn layout(tree: &mut UiTree) {
    tree.layout(VIEWPORT, &mut FixedAdvanceMeasure::default());
}

/// A 200 by 120 window at (100, 100): a 20-unit drag handle holding a
/// title, then a button. (tree, root, handle, button), laid out.
fn window() -> (UiTree, WidgetId, WidgetId, WidgetId) {
    let mut tree = UiTree::new();
    let root = tree.add_root(UiLayer::Debug, at(100.0, 100.0, 200.0, 120.0));
    let handle = tree.panel(root, fixed(200.0, 20.0));
    tree.set_drag_handle(handle, true);
    tree.label(handle, "title");
    let button = tree.button(root, "ok");
    tree.set_layout_style(button, fixed(80.0, 30.0));
    layout(&mut tree);
    (tree, root, handle, button)
}

/// A hand-built world with what the router reads, a frame at a time.
struct Rig {
    world: World,
}

impl Rig {
    fn new(tree: UiTree) -> Self {
        let mut world = World::new();
        world.insert_resource(InputState::new());
        world.insert_resource(ActionMap::default_map());
        world.insert_resource(Time::new());
        world.insert_resource(ScaleFactor::default());
        world.insert_resource(UiRouter::default());
        world.insert_resource(RoutedInput::default());
        world.register_event::<UiEvent>();
        world.insert_resource(tree);
        Self { world }
    }

    fn frame(&mut self, feed: impl FnOnce(&mut InputState)) -> Vec<UiEvent> {
        self.world.flush_events();
        let input = self.world.resource_mut::<InputState>();
        input.begin_frame();
        feed(input);
        self.world.resource_mut::<Time>().advance_frame(DT);
        ui_route_system(&mut self.world);
        layout(self.world.resource_mut::<UiTree>());
        self.world
            .resource::<EventQueue<UiEvent>>()
            .iter_current()
            .copied()
            .collect()
    }

    fn press(&mut self, x: f32, y: f32) -> Vec<UiEvent> {
        self.frame(|input| {
            input.update_cursor_position(x, y);
            input.mouse_down(LEFT);
        })
    }

    fn move_to(&mut self, x: f32, y: f32) -> Vec<UiEvent> {
        self.frame(|input| input.update_cursor_position(x, y))
    }

    fn release(&mut self) -> Vec<UiEvent> {
        self.frame(|input| input.mouse_up(LEFT))
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
}

/// The events that end presses and drags, in order.
fn endings(events: &[UiEvent]) -> Vec<UiEvent> {
    events
        .iter()
        .copied()
        .filter(|event| {
            matches!(
                event,
                UiEvent::Released { .. }
                    | UiEvent::PressCancelled { .. }
                    | UiEvent::Activated { .. }
                    | UiEvent::DoubleClicked { .. }
                    | UiEvent::WindowMoved { .. }
            )
        })
        .collect()
}

#[test]
fn the_pointer_moving_within_the_threshold_moves_nothing() {
    let (tree, root, handle, _button) = window();
    let mut rig = Rig::new(tree);
    rig.press(150.0, 110.0);
    rig.move_to(153.0, 110.0);
    rig.move_to(150.0, 114.0);
    assert_eq!(
        rig.tree().window_position(root),
        Some(Vec2::new(100.0, 100.0)),
        "4 is not past 4"
    );
    assert_eq!(rig.tree().pressed(), Some(handle));
    let events = rig.release();
    assert_eq!(
        endings(&events),
        [
            UiEvent::Released {
                widget: handle,
                button: LEFT
            },
            UiEvent::Activated { widget: handle },
        ],
        "a press that never dragged clicks"
    );
}

#[test]
fn past_the_threshold_the_root_follows_the_pointer_since_the_press() {
    let (tree, root, _handle, _button) = window();
    let mut rig = Rig::new(tree);
    rig.press(150.0, 110.0);
    rig.move_to(160.0, 125.0);
    assert_eq!(
        rig.tree().window_position(root),
        Some(Vec2::new(110.0, 115.0))
    );
    rig.move_to(140.0, 90.0);
    assert_eq!(
        rig.tree().window_position(root),
        Some(Vec2::new(90.0, 80.0)),
        "the motion since the press, not since the last move"
    );
    assert_eq!(
        rig.tree().layout_style(root).unwrap().placement,
        Placement::Anchored {
            anchor: Anchor::TopStart,
            offset: Vec2::new(90.0, 80.0),
        }
    );
    assert_eq!(
        rig.tree().rect(root).unwrap().size(),
        Vec2::new(200.0, 120.0)
    );
}

#[test]
fn a_release_after_a_drag_sends_window_moved_and_activates_nothing() {
    let (tree, root, handle, _button) = window();
    let mut rig = Rig::new(tree);
    rig.press(150.0, 110.0);
    rig.move_to(170.0, 110.0);
    let events = rig.release();
    assert_eq!(
        endings(&events),
        [
            UiEvent::Released {
                widget: handle,
                button: LEFT
            },
            UiEvent::WindowMoved { window: root },
        ]
    );
    assert_eq!(UiEvent::WindowMoved { window: root }.widget(), root);
    // The next click on the handle, without a drag, clicks once again.
    let events = rig.frame(|input| {
        input.mouse_down(LEFT);
        input.mouse_up(LEFT);
    });
    assert_eq!(
        endings(&events),
        [
            UiEvent::Released {
                widget: handle,
                button: LEFT
            },
            UiEvent::Activated { widget: handle },
        ],
        "the drag left no click to pair a double click with"
    );
}

#[test]
fn a_click_on_a_handle_without_a_drag_activates_it() {
    let (tree, _root, handle, _button) = window();
    let mut rig = Rig::new(tree);
    let events = rig.frame(|input| {
        input.update_cursor_position(150.0, 110.0);
        input.mouse_down(LEFT);
        input.mouse_up(LEFT);
    });
    assert_eq!(
        endings(&events),
        [
            UiEvent::Released {
                widget: handle,
                button: LEFT
            },
            UiEvent::Activated { widget: handle },
        ]
    );
}

#[test]
fn focus_loss_or_the_pointer_leaving_mid_drag_leaves_the_window_where_it_is() {
    let (tree, root, handle, _button) = window();
    let mut rig = Rig::new(tree);
    rig.press(150.0, 110.0);
    rig.move_to(170.0, 130.0);
    let events = rig.frame(|input| input.set_focused(false));
    assert_eq!(
        endings(&events),
        [
            UiEvent::PressCancelled { widget: handle },
            UiEvent::WindowMoved { window: root },
        ]
    );
    assert_eq!(
        rig.tree().window_position(root),
        Some(Vec2::new(120.0, 120.0))
    );

    rig.frame(|input| input.set_focused(true));
    rig.press(170.0, 130.0);
    let events = rig.frame(InputState::pointer_left);
    assert_eq!(
        endings(&events),
        [UiEvent::PressCancelled { widget: handle }],
        "no drag, no WindowMoved"
    );
    rig.release();

    // The handle removed mid-drag: the root, still in the tree, moved.
    rig.press(170.0, 130.0);
    rig.move_to(180.0, 140.0);
    rig.tree_mut().remove(handle);
    let events = rig.frame(|_| {});
    assert_eq!(
        endings(&events),
        [
            UiEvent::PressCancelled { widget: handle },
            UiEvent::WindowMoved { window: root },
        ]
    );
}

#[test]
fn the_clamp_keeps_a_window_that_fits_inside_and_one_that_does_not_at_the_edge() {
    let (mut tree, root, _handle, _button) = window();
    assert!(tree.set_window_position(root, Vec2::new(350.0, 250.0)));
    layout(&mut tree);
    assert_eq!(tree.window_position(root), Some(Vec2::new(200.0, 180.0)));
    tree.set_window_position(root, Vec2::new(-50.0, -10.0));
    layout(&mut tree);
    assert_eq!(tree.window_position(root), Some(Vec2::ZERO));

    let wide = tree.add_root(UiLayer::Debug, at(0.0, 0.0, 500.0, 100.0));
    layout(&mut tree);
    assert!(tree.set_window_position(wide, Vec2::new(30.0, 50.0)));
    layout(&mut tree);
    assert_eq!(
        tree.window_position(wide),
        Some(Vec2::new(0.0, 50.0)),
        "too wide: its left edge at the viewport's; it fits down"
    );

    // A drag past the viewport's corner stops there.
    let mut rig = Rig::new(tree);
    rig.press(110.0, 10.0);
    rig.move_to(400.0, 300.0);
    assert_eq!(
        rig.tree().window_position(root),
        Some(Vec2::new(200.0, 180.0))
    );
}

#[test]
fn set_window_position_takes_roots_and_finite_positions_and_an_unmeasured_root() {
    let (mut tree, root, handle, _button) = window();
    assert!(
        !tree.set_window_position(handle, Vec2::new(10.0, 10.0)),
        "not a root"
    );
    assert_eq!(tree.layout_style(handle), Some(&fixed(200.0, 20.0)));
    assert_eq!(tree.window_position(handle), None);
    for bad in [Vec2::new(f32::NAN, 0.0), Vec2::new(0.0, f32::INFINITY)] {
        assert!(!tree.set_window_position(root, bad));
    }
    assert!(!tree.is_layout_dirty());
    assert!(tree.set_window_position(root, Vec2::new(20.0, 30.0)));
    assert!(tree.is_layout_dirty());
    assert!(
        !tree.set_window_position(root, Vec2::new(20.0, 30.0)),
        "unchanged"
    );

    // A root with no size yet keeps its top-left 48 inside the viewport.
    let fresh = tree.add_root(UiLayer::Debug, fixed(150.0, 90.0));
    assert_eq!(tree.window_position(fresh), None, "before its first layout");
    assert!(tree.set_window_position(fresh, Vec2::new(390.0, 290.0)));
    assert_eq!(
        tree.layout_style(fresh).unwrap().placement,
        Placement::Anchored {
            anchor: Anchor::TopStart,
            offset: Vec2::new(352.0, 252.0),
        }
    );

    // With no layout at all, only at least 0.
    let mut empty = UiTree::new();
    let lone = empty.add_root(UiLayer::Debug, LayoutStyle::default());
    assert!(empty.set_window_position(lone, Vec2::new(-5.0, 7000.0)));
    assert_eq!(
        empty.layout_style(lone).unwrap().placement,
        Placement::Anchored {
            anchor: Anchor::TopStart,
            offset: Vec2::new(0.0, 7000.0),
        }
    );
}

#[test]
fn a_press_on_a_handle_leaves_focus_alone_and_shows_the_move_cursor() {
    let (mut tree, _root, handle, button) = window();
    assert!(tree.drag_handle(handle));
    assert!(!tree.drag_handle(button));
    assert!(!tree.focusable(handle));
    assert!(tree.focus(button));
    let mut rig = Rig::new(tree);
    let events = rig.press(150.0, 110.0);
    assert!(events.contains(&UiEvent::Pressed {
        widget: handle,
        button: LEFT
    }));
    assert_eq!(
        rig.tree().focused(),
        Some(button),
        "the handle takes no focus"
    );
    assert_eq!(
        rig.router().cursor(),
        CursorIcon::Move,
        "over the title, inside the handle"
    );
    rig.release();
    rig.tree_mut().set_cursor(handle, Some(CursorIcon::Pointer));
    rig.move_to(150.0, 111.0);
    assert_eq!(
        rig.router().cursor(),
        CursorIcon::Pointer,
        "a set cursor wins"
    );
    rig.tree_mut().set_drag_handle(handle, false);
    rig.tree_mut().set_cursor(handle, None);
    rig.move_to(150.0, 112.0);
    assert_eq!(rig.router().cursor(), CursorIcon::Default);
    assert!(!rig.tree().drag_handle(handle));
}

fn tempdir() -> PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let nonce = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "tungsten_window_test_{}_{}",
        std::process::id(),
        nonce
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// `text` written to a fresh `debug_windows.json`, then loaded.
fn load_text(text: &str) -> (PathBuf, WindowPlacements) {
    let path = tempdir().join("debug_windows.json");
    std::fs::write(&path, text).unwrap();
    let placements = WindowPlacements::load(&path);
    (path, placements)
}

#[test]
fn placements_round_trip_through_the_file() {
    let path = tempdir().join("debug_windows.json");
    let mut placements = WindowPlacements::load(&path);
    assert!(
        placements.take_load_warnings().is_empty(),
        "a missing file is no error"
    );
    assert_eq!(placements.get("hud"), None);
    assert!(placements.set("hud", Vec2::new(12.5, 30.25)));
    assert!(placements.set("systems", Vec2::new(100.0, 200.0)));
    assert!(
        !placements.set("hud", Vec2::new(f32::NAN, 1.0)),
        "kept as it was"
    );
    placements.save().unwrap();

    let text = std::fs::read_to_string(&path).unwrap();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(json["version"], 1);
    assert_eq!(json["windows"]["hud"], serde_json::json!([12.5, 30.25]));
    let mut loaded = WindowPlacements::load(&path);
    assert!(loaded.take_load_warnings().is_empty());
    assert_eq!(loaded.get("hud"), Some(Vec2::new(12.5, 30.25)));
    assert_eq!(loaded.get("systems"), Some(Vec2::new(100.0, 200.0)));

    let mut pathless = WindowPlacements::default();
    assert!(pathless.set("hud", Vec2::ONE));
    pathless.save().unwrap();
    assert_eq!(pathless.get("hud"), Some(Vec2::ONE));
}

#[test]
fn an_invalid_file_loads_empty_and_is_renamed_bad_with_one_warning() {
    for text in [
        "not json",
        "[1, 2]",
        r#"{"windows": {}}"#,
        r#"{"version": 0, "windows": {}}"#,
        r#"{"version": 1.5, "windows": {}}"#,
        r#"{"version": 1, "windows": [1, 2]}"#,
    ] {
        let (path, mut placements) = load_text(text);
        let warnings = placements.take_load_warnings();
        assert_eq!(warnings.len(), 1, "{text}: {warnings:?}");
        assert!(
            warnings[0].contains("debug_windows.json.bad"),
            "{}",
            warnings[0]
        );
        assert!(!path.exists(), "{text}");
        let bad = path.with_file_name("debug_windows.json.bad");
        assert_eq!(std::fs::read_to_string(bad).unwrap(), text);
        assert_eq!(placements.get("hud"), None);
    }
}

#[test]
fn a_newer_version_loads_empty_and_leaves_the_file_as_it_is() {
    let text = r#"{"version": 2, "windows": {"hud": [1, 2]}}"#;
    let (path, mut placements) = load_text(text);
    let warnings = placements.take_load_warnings();
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("version 2"), "{}", warnings[0]);
    assert_eq!(placements.get("hud"), None);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    assert!(!path.with_file_name("debug_windows.json.bad").exists());
}

#[test]
fn entries_that_are_not_two_finite_numbers_are_dropped_with_a_warning_each() {
    let (path, mut placements) = load_text(
        r#"{"version": 1, "extra": true, "windows": {
            "hud": [1, 2],
            "short": [1],
            "text": "x",
            "mixed": [1, "y"],
            "huge": [1e39, 0],
            "long": [1, 2, 3]
        }}"#,
    );
    let warnings = placements.take_load_warnings();
    assert_eq!(warnings.len(), 5, "{warnings:?}");
    for name in ["short", "text", "mixed", "huge", "long"] {
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains(&format!("'{name}'"))),
            "{name}"
        );
        assert_eq!(placements.get(name), None);
    }
    assert_eq!(placements.get("hud"), Some(Vec2::new(1.0, 2.0)));
    assert!(path.exists(), "the file stays; a save rewrites it");
    assert!(placements.take_load_warnings().is_empty(), "taken once");
}
