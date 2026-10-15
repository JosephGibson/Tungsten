use tungsten_core::ui::{
    Anchor, CursorIcon, InputDevice, LayoutStyle, Sizing, SliderRange, UI_RIGHT, UiLayer, UiValue,
};
use tungsten_core::{
    ActionMap, Binding, Config, EventQueue, GamepadButton, GamepadId, InputState, KeyCode,
    MouseButton, RoutedInput, Stage, UiEvent, UiRouter, system,
};

use super::*;
use crate::app::WindowSize;
use crate::plugins::{DefaultPlugins, UiPlugin};

fn app() -> App {
    App::new(Config::default()).expect("App::new failed")
}

fn at(x: f32, y: f32, w: f32, h: f32) -> LayoutStyle {
    LayoutStyle::default()
        .with_size(Sizing::Px(w), Sizing::Px(h))
        .anchored(Anchor::TopStart, Vec2::new(x, y))
}

/// A root at the window's top left holding one 100 by 40 button at
/// (20, 20), laid out.
fn button(harness: &mut UiHarness) -> WidgetId {
    let tree = harness.tree_mut();
    let root = tree.add_root(UiLayer::Game, at(0.0, 0.0, 200.0, 100.0));
    let button = tree.button(root, "Start");
    tree.set_layout_style(button, at(20.0, 20.0, 100.0, 40.0));
    harness.layout();
    button
}

fn routed(harness: &UiHarness) -> &RoutedInput {
    harness.world().resource::<RoutedInput>()
}

struct Watched(WidgetId);
struct Count(u32);

fn count_activations(world: &mut World) {
    let widget = world.resource::<Watched>().0;
    if world.resource::<EventQueue<UiEvent>>().activated(widget) {
        world.resource_mut::<Count>().0 += 1;
    }
}

fn count_jumps(world: &mut World) {
    let jumped = world
        .resource::<ActionMap>()
        .just_pressed(world.resource::<RoutedInput>(), "jump");
    if jumped {
        world.resource_mut::<Count>().0 += 1;
    }
}

#[test]
fn an_update_system_sees_one_activation_per_click() {
    let mut app = app();
    app.add_system_named("count_activations", count_activations);
    let mut harness = UiHarness::new(app);
    let button = button(&mut harness);
    assert_eq!(
        harness.find("Start"),
        Some(button),
        "the button before its label"
    );
    harness.world_mut().insert_resource(Watched(button));
    harness.world_mut().insert_resource(Count(0));
    harness.click(button);
    harness.step(1);
    assert_eq!(harness.world().resource::<Count>().0, 1);
    harness.step(3);
    assert_eq!(
        harness.world().resource::<Count>().0,
        1,
        "and in no later frame"
    );
    harness.click(button);
    harness.step(1);
    assert_eq!(harness.world().resource::<Count>().0, 2);
}

#[test]
fn a_fixed_update_system_on_the_routed_view_sees_each_press_in_one_step() {
    for hz in [144.0, 30.0] {
        let mut app = app();
        app.add_system_to(Stage::FixedUpdate, system("count_jumps", count_jumps));
        let mut harness = UiHarness::new(app);
        harness.harness_mut().set_dt(1.0 / hz);
        harness.world_mut().insert_resource(Count(0));
        for press in 1..=5 {
            harness.key_down(KeyCode::Space);
            harness.step(1);
            harness.key_up(KeyCode::Space);
            harness.step(6);
            assert_eq!(
                harness.world().resource::<Count>().0,
                press,
                "press {press} at {hz} Hz"
            );
        }
    }
}

#[test]
fn a_click_on_a_button_never_reaches_the_routed_view_and_one_beside_the_ui_does() {
    let mut harness = UiHarness::new(app());
    let button = button(&mut harness);
    harness.click(button);
    harness.step(1);
    assert!(!routed(&harness).mouse_just_pressed(MouseButton::Left));
    assert!(!routed(&harness).mouse_just_released(MouseButton::Left));
    harness.pointer_to(600.0, 400.0);
    harness.pointer_down();
    harness.pointer_up();
    harness.step(1);
    assert!(routed(&harness).mouse_just_pressed(MouseButton::Left));
    assert!(routed(&harness).mouse_just_released(MouseButton::Left));
    assert!(!harness.world().resource::<UiRouter>().pointer_over_ui());
}

#[test]
fn escape_with_visible_focus_does_not_exit_and_without_focus_does() {
    let mut harness = UiHarness::new(app());
    let button = button(&mut harness);
    assert!(harness.focus(button));
    harness.key_down(KeyCode::Escape);
    harness.step(1);
    assert!(!harness.harness().exit_requested(), "the UI took it");
    assert_eq!(harness.tree().focused(), None);
    harness.key_up(KeyCode::Escape);
    harness.step(1);
    harness.key_down(KeyCode::Escape);
    harness.step(1);
    assert!(harness.harness().exit_requested(), "without focus: exit");
}

/// Per frame, what `InputState` held when `update` ran.
struct Seen(Vec<String>);

fn record_input(world: &mut World) {
    let input = world.resource::<InputState>();
    let keys = [
        KeyCode::Space,
        KeyCode::Enter,
        KeyCode::Tab,
        KeyCode::ArrowDown,
        KeyCode::KeyW,
    ];
    let buttons = [MouseButton::Left, MouseButton::Right];
    let line = format!(
        "{:?} {:?} {:?} {:?} {:?} {:?} {:?}",
        input.raw_events(),
        keys.map(|key| (
            input.is_pressed(key),
            input.just_pressed(key),
            input.just_released(key)
        )),
        buttons.map(|button| (
            input.is_mouse_pressed(button),
            input.mouse_just_pressed(button),
            input.mouse_just_released(button)
        )),
        input.cursor_position(),
        input.cursor_delta(),
        input.modifiers(),
        (input.focused(), input.pointer_in_window()),
    );
    world.resource_mut::<Seen>().0.push(line);
}

/// The same scripted input through `app`, recording `InputState` each
/// frame.
fn scripted(mut app: App) -> (Vec<String>, UiHarness) {
    app.add_system_named("record_input", record_input);
    let mut harness = UiHarness::new(app);
    harness.world_mut().insert_resource(Seen(Vec::new()));
    let button = button(&mut harness);
    harness.click(button);
    harness.step(1);
    harness.key_down(KeyCode::Space);
    harness.step(1);
    harness.key_down(KeyCode::Enter);
    harness.key_down(KeyCode::Tab);
    harness.step(1);
    harness.key_up(KeyCode::Enter);
    harness.key_up(KeyCode::Space);
    harness.key_up(KeyCode::Tab);
    harness.key_down(KeyCode::ArrowDown);
    harness.step(2);
    harness.key_up(KeyCode::ArrowDown);
    harness.pointer_to(500.0, 300.0);
    harness.pointer_down();
    harness.step(1);
    harness.pointer_up();
    let input = harness.world_mut().resource_mut::<InputState>();
    input.add_scroll_line_delta(0.0, 1.0);
    input.set_focused(false);
    harness.step(1);
    harness
        .world_mut()
        .resource_mut::<InputState>()
        .set_focused(true);
    harness.step(1);
    let seen = std::mem::take(&mut harness.world_mut().resource_mut::<Seen>().0);
    (seen, harness)
}

#[test]
fn routing_leaves_the_raw_input_as_it_is_without_the_ui_plugin() {
    let (with, routed_run) = scripted(app());
    let without_plugin = App::with_plugins(
        Config::default(),
        DefaultPlugins::set().without::<UiPlugin>(),
    )
    .expect("App::with_plugins failed");
    let (without, plain_run) = scripted(without_plugin);
    assert_eq!(with.len(), 8);
    assert_eq!(with, without);
    let device = |run: &UiHarness| run.world().resource::<UiRouter>().last_device();
    assert_eq!(device(&routed_run), InputDevice::Pointer, "one run routed");
    assert_eq!(
        device(&plain_run),
        InputDevice::Keyboard,
        "the other did not"
    );
}

#[test]
fn the_window_cursor_follows_the_hovered_widget() {
    let mut harness = UiHarness::new(app());
    let button = button(&mut harness);
    harness
        .tree_mut()
        .set_cursor(button, Some(CursorIcon::Pointer));
    let rect = harness.rect(button).unwrap();
    let centre = (rect.min + rect.max) * 0.5;
    harness.pointer_to(centre.x, centre.y);
    harness.step(1);
    assert_eq!(
        harness.world().resource::<UiRouter>().cursor(),
        CursorIcon::Pointer
    );
    assert_eq!(harness.harness().app.applied_cursor(), CursorIcon::Pointer);
    harness.pointer_to(600.0, 400.0);
    harness.step(1);
    assert_eq!(harness.harness().app.applied_cursor(), CursorIcon::Default);
}

#[test]
fn at_scale_2_the_harness_lays_out_logically_and_a_physical_click_lands() {
    let mut harness = UiHarness::new(app());
    harness
        .world_mut()
        .resource_mut::<tungsten_core::ScaleFactor>()
        .set_os(2.0);
    let size = *harness.world().resource::<WindowSize>();
    let logical = Vec2::new(size.width as f32, size.height as f32) / 2.0;
    let tree = harness.tree_mut();
    let screen = tree.add_root(
        UiLayer::Game,
        LayoutStyle::default().with_size(Sizing::Percent(100.0), Sizing::Percent(100.0)),
    );
    let button = tree.button(screen, "Start");
    tree.set_layout_style(button, at(100.0, 100.0, 100.0, 40.0));
    harness.layout();
    assert_eq!(harness.rect(screen).unwrap().max, logical);
    assert_eq!(
        harness.hit(360.0, 260.0),
        Some(button),
        "pixel (360, 260) is (180, 130), past the label"
    );
    harness.click(button);
    harness.step(1);
    assert!(
        harness
            .world()
            .resource::<EventQueue<UiEvent>>()
            .iter()
            .any(|event| matches!(event, UiEvent::Activated { widget, .. } if *widget == button))
    );
}

/// The values `Watched` controls took, as an `Update` system saw them.
#[derive(Default)]
struct Values(Vec<UiValue>);
struct WatchedControls(Vec<WidgetId>);

fn record_values(world: &mut World) {
    let queue = world.resource::<EventQueue<UiEvent>>();
    let seen: Vec<UiValue> = world
        .resource::<WatchedControls>()
        .0
        .iter()
        .filter_map(|widget| queue.value_changed(*widget))
        .collect();
    world.resource_mut::<Values>().0.extend(seen);
}

/// A root at the window's top left holding a slider (0 to 10 by 1, at 5)
/// at (20, 20), 200 by 20, whose thumb's centre runs from x 30 to 210, and
/// a toggle (off) at (20, 60), laid out.
fn controls(harness: &mut UiHarness) -> (WidgetId, WidgetId) {
    let tree = harness.tree_mut();
    let root = tree.add_root(UiLayer::Game, at(0.0, 0.0, 300.0, 100.0));
    let slider = tree.slider(root, SliderRange::new(0.0, 10.0).with_step(1.0), 5.0);
    tree.set_layout_style(slider, at(20.0, 20.0, 200.0, 20.0));
    let toggle = tree.toggle(root, false);
    tree.set_layout_style(toggle, at(20.0, 60.0, 20.0, 20.0));
    harness.layout();
    (slider, toggle)
}

/// The x of the value `value` on the controls' slider.
fn slider_x(value: f32) -> f32 {
    30.0 + 18.0 * value
}

#[test]
fn an_update_system_sees_each_value_change_once() {
    let mut app = app();
    app.add_system_named("record_values", record_values);
    let mut harness = UiHarness::new(app);
    let (slider, toggle) = controls(&mut harness);
    harness
        .world_mut()
        .insert_resource(WatchedControls(vec![slider, toggle]));
    harness.world_mut().insert_resource(Values::default());
    let seen = |harness: &UiHarness| harness.world().resource::<Values>().0.clone();

    // A click.
    harness.pointer_to(slider_x(2.0), 30.0);
    harness.pointer_down();
    harness.pointer_up();
    harness.step(3);
    assert_eq!(seen(&harness), [UiValue::Number(2.0)]);

    // A drag across two frames.
    harness.pointer_to(slider_x(7.0), 30.0);
    harness.pointer_down();
    harness.step(1);
    harness.pointer_to(slider_x(9.0), 30.0);
    harness.step(1);
    harness.pointer_up();
    harness.step(2);
    assert_eq!(
        seen(&harness),
        [
            UiValue::Number(2.0),
            UiValue::Number(7.0),
            UiValue::Number(9.0)
        ]
    );

    // An arrow press made while focus shows: the first key after pointer
    // use is the game's and shows focus.
    harness.key_down(KeyCode::ArrowRight);
    harness.step(1);
    harness.key_up(KeyCode::ArrowRight);
    harness.step(1);
    assert_eq!(harness.tree().slider_value(slider), Some(9.0));
    harness.key_down(KeyCode::ArrowRight);
    harness.step(2);
    harness.key_up(KeyCode::ArrowRight);
    harness.step(1);
    assert_eq!(seen(&harness).last(), Some(&UiValue::Number(10.0)));
    assert_eq!(seen(&harness).len(), 4);

    harness.click(toggle);
    harness.step(2);
    assert_eq!(seen(&harness).last(), Some(&UiValue::Bool(true)));
    assert_eq!(seen(&harness).len(), 5);
}

#[test]
fn a_rebound_key_and_the_dpad_step_a_slider_holding_visible_focus() {
    let mut harness = UiHarness::new(app());
    let (slider, _toggle) = controls(&mut harness);
    assert!(harness.focus(slider));
    let pad = GamepadId::new(0);
    harness
        .harness_mut()
        .input_mut()
        .gamepad_button_down(pad, GamepadButton::DPadRight);
    harness.step(1);
    assert_eq!(harness.tree().slider_value(slider), Some(6.0));
    harness
        .harness_mut()
        .input_mut()
        .gamepad_button_up(pad, GamepadButton::DPadRight);
    harness.step(1);

    harness
        .world_mut()
        .resource_mut::<ActionMap>()
        .replace_bindings(
            UI_RIGHT,
            vec![Binding::Key {
                code: KeyCode::KeyD,
            }],
        );
    harness.key_down(KeyCode::KeyD);
    harness.step(1);
    assert_eq!(harness.tree().slider_value(slider), Some(7.0));
    assert!(!routed(&harness).is_pressed(KeyCode::KeyD), "the UI's key");
    harness.key_up(KeyCode::KeyD);
    harness.key_down(KeyCode::ArrowRight);
    harness.step(1);
    assert_eq!(
        harness.tree().slider_value(slider),
        Some(7.0),
        "the arrow left ui_right"
    );
}
