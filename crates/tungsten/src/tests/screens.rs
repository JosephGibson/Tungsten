use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec2;
use tungsten_core::ui::{
    ScreenFile, UiAction, UiBindings, UiKey, UiRow, UiScreenRegistry, UiTree, UiValue, widgets,
};
use tungsten_core::{Config, KeyCode, Settings, World};

use super::*;
use crate::App;
use crate::state::{StateContext, StateId};
use crate::testing::UiHarness;

type Heard = Rc<RefCell<Vec<String>>>;

/// A screen of keyed buttons on a [`ModalScreen`]: it records each key its
/// update hands it, and on `open` pushes `then` when one is set.
struct Menu {
    id: StateId,
    buttons: &'static [(&'static str, &'static str)],
    heard: Heard,
    screen: Option<ModalScreen>,
    then: Option<fn(&Heard) -> Menu>,
}

impl Menu {
    fn new(id: StateId, buttons: &'static [(&'static str, &'static str)], heard: &Heard) -> Self {
        Self {
            id,
            buttons,
            heard: heard.clone(),
            screen: None,
            then: None,
        }
    }
}

impl GameState for Menu {
    fn id(&self) -> StateId {
        self.id
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        let screen = ModalScreen::open(ctx.world, 260.0);
        let tree = ctx.world.resource_mut::<UiTree>();
        for (key, caption) in self.buttons {
            let button = widgets::button(tree, screen.card, caption);
            tree.set_key(button, key);
        }
        screen.resume(ctx.world);
        self.screen = Some(screen);
    }

    fn on_exit(&mut self, _ctx: &mut StateContext) {}

    fn on_resume(&mut self, ctx: &mut StateContext) {
        if let Some(screen) = &self.screen {
            screen.resume(ctx.world);
        }
    }

    fn update(&mut self, world: &mut World) {
        let Some(mut screen) = self.screen else {
            return;
        };
        let key = screen.update(world);
        self.screen = Some(screen);
        let Some(key) = key else {
            return;
        };
        let name = self
            .buttons
            .iter()
            .find(|(name, _)| UiKey::new(name) == key)
            .map_or("?", |(name, _)| *name);
        self.heard.borrow_mut().push(format!("{}:{name}", self.id));
        if name == "open"
            && let Some(then) = self.then
        {
            push(world, then(&self.heard));
        }
    }
}

const PLAIN: &[(&str, &str)] = &[("open", "Open"), ("other", "Other")];

fn child(heard: &Heard) -> Menu {
    Menu::new("child", PLAIN, heard)
}

fn harness() -> (UiHarness, Heard) {
    let mut app = App::new(Config::default()).expect("App::new failed");
    app.add_plugin(ScreensPlugin);
    (UiHarness::new(app), Rc::new(RefCell::new(Vec::new())))
}

fn stack(harness: &UiHarness) -> &StateStack {
    harness.world().resource::<StateStack>()
}

/// The front-most modal root.
fn front(harness: &UiHarness) -> WidgetId {
    let tree = harness.tree();
    tree.roots()
        .iter()
        .rev()
        .copied()
        .find(|root| tree.modal(*root))
        .expect("a modal root")
}

/// Opens `menu` with a plain push and lets its slide in finish.
fn opened(harness: &mut UiHarness, menu: Menu) {
    harness
        .world_mut()
        .resource_mut::<StateStack>()
        .request_push(menu);
    harness.step(12);
}

fn tap(harness: &mut UiHarness, key: KeyCode) {
    harness.key_down(key);
    harness.step(1);
    harness.key_up(key);
    harness.step(1);
}

#[test]
fn a_pushed_screen_slides_in_and_takes_focus() {
    let (mut harness, heard) = harness();
    harness
        .world_mut()
        .resource_mut::<StateStack>()
        .request_push(Menu::new("menu", PLAIN, &heard));
    harness.step(1);
    let root = front(&harness);
    let tree = harness.tree();
    assert!(
        tree.opacity(root).is_some_and(|alpha| alpha < 1.0),
        "fading in"
    );
    assert!(
        tree.translation(root).is_some_and(|at| at.y > 0.0),
        "sliding up"
    );
    let first = harness.find("Open").expect("its button");
    assert_eq!(
        harness.tree().focused(),
        Some(first),
        "focus on its first control"
    );
    harness.step(12);
    let tree = harness.tree();
    assert_eq!(tree.opacity(root), Some(1.0));
    assert_eq!(tree.translation(root), Some(Vec2::ZERO));
}

#[test]
fn cancel_pops_the_front_screen_after_its_slide() {
    let (mut harness, heard) = harness();
    opened(&mut harness, Menu::new("menu", PLAIN, &heard));
    opened(&mut harness, child(&heard));
    assert_eq!(stack(&harness).depth(), 2);
    let root = front(&harness);
    harness.key_down(KeyCode::Escape);
    harness.step(1);
    harness.key_up(KeyCode::Escape);
    assert!(transitioning(harness.world()), "sliding out");
    assert_eq!(stack(&harness).depth(), 2, "it pops after its slide");
    harness.step(2);
    assert!(
        harness
            .tree()
            .opacity(root)
            .is_some_and(|alpha| alpha < 1.0)
    );
    harness.step(8);
    assert_eq!(stack(&harness).depth(), 1);
    assert_eq!(stack(&harness).active_id(), Some("menu"));
    assert!(idle(harness.world()));
}

#[test]
fn a_back_button_pops() {
    const BACKED: &[(&str, &str)] = &[("other", "Other"), ("back", "Back")];
    let (mut harness, heard) = harness();
    opened(&mut harness, Menu::new("menu", PLAIN, &heard));
    opened(&mut harness, Menu::new("child", BACKED, &heard));
    let back = harness.find("Back").expect("the back button");
    harness.click(back);
    harness.step(12);
    assert_eq!(stack(&harness).depth(), 1, "its cancel popped the screen");
    assert!(heard.borrow().is_empty(), "the back key reaches no handler");
}

#[test]
fn focus_returns_to_the_opener_on_resume() {
    let (mut harness, heard) = harness();
    let mut menu = Menu::new("menu", PLAIN, &heard);
    menu.then = Some(child);
    opened(&mut harness, menu);
    let open = harness.find("Open").expect("the opener");
    harness.click(open);
    harness.step(12);
    assert_eq!(stack(&harness).depth(), 2);
    assert_eq!(*heard.borrow(), ["menu:open"]);
    tap(&mut harness, KeyCode::Escape);
    harness.step(12);
    assert_eq!(stack(&harness).depth(), 1);
    assert_eq!(harness.tree().focused(), Some(open), "back on the opener");
}

#[test]
fn a_screen_acts_only_when_idle() {
    let (mut harness, heard) = harness();
    opened(&mut harness, Menu::new("menu", PLAIN, &heard));
    let other = harness.find("Other").expect("a button");
    // A capture pending: the screen hears nothing.
    harness
        .world_mut()
        .resource_mut::<tungsten_core::UiRouter>()
        .capture_next_input(other);
    assert!(!idle(harness.world()));
    harness
        .world_mut()
        .resource_mut::<tungsten_core::UiRouter>()
        .cancel_capture();
    harness.step(1);
    // A change waiting: an activation goes unheard.
    harness
        .world_mut()
        .resource_mut::<StateStack>()
        .request_push(child(&heard));
    assert!(!idle(harness.world()));
    harness.step(12);
    assert!(heard.borrow().is_empty());
    tap(&mut harness, KeyCode::Escape);
    harness.step(12);
    harness.click(other);
    harness.step(1);
    assert_eq!(*heard.borrow(), ["menu:other"], "idle, it acts");
}

#[test]
fn one_change_runs_at_a_time() {
    let (mut harness, heard) = harness();
    opened(&mut harness, Menu::new("menu", PLAIN, &heard));
    let world = harness.world_mut();
    assert!(push(world, child(&heard)));
    assert!(
        !push(world, child(&heard)),
        "a second change waits for none"
    );
    assert!(!pop(world));
    harness.step(12);
    assert_eq!(stack(&harness).depth(), 2);
    let world = harness.world_mut();
    assert!(pop(world));
    assert!(!pop(world), "one pop slides at a time");
    assert!(!push(world, child(&heard)));
    harness.step(12);
    assert_eq!(stack(&harness).depth(), 1);
}

/// A marker the reset's swap inserts.
struct Swapped;

#[test]
fn a_reset_swaps_the_run_under_its_fade() {
    let (mut harness, heard) = harness();
    opened(&mut harness, Menu::new("menu", PLAIN, &heard));
    opened(&mut harness, child(&heard));
    let world = harness.world_mut();
    assert!(reset(world, Menu::new("title", PLAIN, &heard), |world| {
        world.insert_resource(Swapped);
    }));
    assert!(!idle(harness.world()));
    harness.step(3);
    let covered = front(&harness);
    assert!(
        harness.tree().fade(covered).is_some_and(|fade| fade < 1.0),
        "the screens fade out"
    );
    assert!(!harness.world().has_resource::<Swapped>());
    harness.step(4);
    assert!(
        harness.world().has_resource::<Swapped>(),
        "swapped on the covered frame"
    );
    assert_eq!(stack(&harness).depth(), 1);
    assert_eq!(stack(&harness).active_id(), Some("title"));
    harness.step(10);
    assert!(idle(harness.world()));
}

/// The answers a confirm took, in order.
fn asked(world: &mut World, heard: &Heard, countdown: Option<f32>) {
    let (yes, no) = (heard.clone(), heard.clone());
    let mut confirm = Confirm::new("Quit?", "Quit", "Stay")
        .on_yes(move |_| yes.borrow_mut().push("yes".to_owned()))
        .on_no(move |_| no.borrow_mut().push("no".to_owned()));
    if let Some(secs) = countdown {
        confirm = confirm.countdown(secs);
    }
    assert!(ask(world, confirm));
}

#[test]
fn confirm_takes_the_safe_answer_on_cancel_countdown_and_exit() {
    let (mut harness, heard) = harness();
    opened(&mut harness, Menu::new("menu", PLAIN, &heard));
    asked(harness.world_mut(), &heard, None);
    harness.step(12);
    assert_eq!(stack(&harness).active_id(), Some(CONFIRM));
    assert_eq!(
        harness.tree().focused(),
        harness.find("Stay"),
        "the safe answer focused"
    );
    tap(&mut harness, KeyCode::Escape);
    harness.step(12);
    assert_eq!(*heard.borrow(), ["no"], "a cancel");
    assert_eq!(stack(&harness).active_id(), Some("menu"));

    asked(harness.world_mut(), &heard, Some(0.5));
    harness.step(12);
    harness.step(30);
    assert_eq!(*heard.borrow(), ["no", "no"], "the countdown's end");
    harness.step(12);
    assert_eq!(stack(&harness).active_id(), Some("menu"));

    asked(harness.world_mut(), &heard, None);
    harness.step(12);
    crate::request_exit(harness.world_mut());
    harness.step(2);
    assert_eq!(*heard.borrow(), ["no", "no", "no"], "the exit");
}

#[test]
fn the_first_answer_is_final() {
    let (mut harness, heard) = harness();
    opened(&mut harness, Menu::new("menu", PLAIN, &heard));
    asked(harness.world_mut(), &heard, Some(0.5));
    harness.step(12);
    let quit = harness.find("Quit").expect("the answer");
    harness.click(quit);
    harness.step(1);
    assert_eq!(*heard.borrow(), ["yes"]);
    // Sliding out, a cancel, the countdown's end and the exit answer
    // nothing more.
    harness.key_down(KeyCode::Escape);
    crate::request_exit(harness.world_mut());
    harness.step(40);
    assert_eq!(*heard.borrow(), ["yes"]);
}

#[test]
fn toasts_stay_above_a_reset_and_leave_on_the_real_clock() {
    let (mut harness, heard) = harness();
    opened(&mut harness, Menu::new("menu", PLAIN, &heard));
    toast(harness.world_mut(), "Saved");
    harness.step(1);
    let toasts = |harness: &UiHarness| harness.world().resource::<Toasts>().texts(harness.tree());
    assert_eq!(toasts(&harness), ["Saved"]);
    assert!(reset(
        harness.world_mut(),
        Menu::new("title", PLAIN, &heard),
        |_| {}
    ));
    harness.step(3);
    let tree = harness.tree();
    let toast_root = *tree.roots().last().expect("roots");
    assert!(
        !tree.modal(toast_root),
        "the toasts are front-most, over the screens"
    );
    assert_eq!(
        tree.fade(toast_root),
        Some(1.0),
        "unowned, they do not fade"
    );
    assert_eq!(toasts(&harness), ["Saved"]);
    // A paused game clock does not hold them: 2.5 s of real time does.
    harness
        .world_mut()
        .resource_mut::<tungsten_core::Time>()
        .pause();
    harness.step(150);
    harness.step(3);
    assert!(toasts(&harness).is_empty(), "gone after 2.5 s");
    toast(harness.world_mut(), "Again");
    assert_eq!(toasts(&harness), ["Again"], "and the next one shows alone");
}

#[test]
fn a_registered_screen_opens_by_id() {
    let (mut harness, _heard) = harness();
    fn settings() -> Box<dyn GameState> {
        Box::new(Menu::new(
            "settings",
            PLAIN,
            &Rc::new(RefCell::new(Vec::new())),
        ))
    }
    let world = harness.world_mut();
    register(world, "settings", settings);
    assert_eq!(world.resource::<ScreenRegistry>().ids(), ["settings"]);
    assert!(open(world, "settings"));
    assert!(!open(world, "nowhere"), "an unknown ID opens nothing");
    harness.step(1);
    assert_eq!(stack(&harness).active_id(), Some("settings"));
}

/// The idle-screen guard (redo 7.17): a screen past its entrance lays out
/// nothing while nothing changes; a focus move only repaints, and one
/// label's new text lays out its root alone.
#[test]
fn an_idle_screen_lays_out_no_root() {
    let (mut harness, heard) = harness();
    opened(&mut harness, Menu::new("menu", PLAIN, &heard));
    let before = harness.layout_totals();
    harness.step(60);
    assert_eq!(
        harness.layout_totals(),
        before,
        "60 frames without input lay nothing out"
    );
    let focused = harness.tree().focused();
    assert!(
        harness
            .focus_next()
            .is_some_and(|next| Some(next) != focused)
    );
    harness.step(1);
    assert_eq!(
        harness.layout_totals(),
        before,
        "a focused look repaints only"
    );
    let other = harness.find("Other").expect("a button");
    assert!(harness.tree_mut().set_plain_text(other, "Others"));
    harness.step(1);
    assert_eq!(
        harness.layout_totals().roots_laid_out,
        before.roots_laid_out + 1,
        "a new text lays out its root"
    );
}

// ---- data screens (redo 7.20) ----

/// What the data screens' bindings read, and the actions they heard.
#[derive(Debug, Default)]
struct Board {
    text: String,
    shown: bool,
    enabled: bool,
    on: bool,
    heard: Vec<UiAction>,
}

fn board(world: &World) -> &Board {
    world.resource::<Board>()
}

fn board_text(world: &World) -> String {
    board(world).text.clone()
}

fn board_shown(world: &World) -> bool {
    board(world).shown
}

fn board_enabled(world: &World) -> bool {
    board(world).enabled
}

fn board_on(world: &World) -> UiValue {
    UiValue::Bool(board(world).on)
}

fn board_rows(_world: &World) -> Vec<UiRow> {
    vec![UiRow::new("a"), UiRow::new("b")]
}

fn record(world: &mut World, action: UiAction) {
    world.resource_mut::<Board>().heard.push(action);
}

/// A screen file whose card, 260 units wide, holds `children`.
fn file(children: &str) -> String {
    format!(
        r#"{{"version": 1, "root": {{"kind": "screen", "width": 260, "children": [{children}]}}}}"#
    )
}

/// A harness whose `ui_screens` are `files` (ID, JSON), with the board's
/// bindings and actions; `plugin` adds [`ScreensPlugin`].
fn data_harness(files: &[(&str, &str)], plugin: bool) -> UiHarness {
    let mut app = App::new(Config::default()).expect("App::new failed");
    if plugin {
        app.add_plugin(ScreensPlugin);
    }
    let world = app.world_mut();
    let mut registry = UiScreenRegistry::new();
    for (id, json) in files {
        let file = ScreenFile::from_json(json).unwrap_or_else(|e| panic!("{id}: {e}"));
        registry.insert(*id, file);
    }
    world.insert_resource(registry);
    world.insert_resource(Board {
        shown: true,
        enabled: true,
        ..Board::default()
    });
    UiBindings::of(world)
        .text("text", board_text)
        .visible("shown", board_shown)
        .enabled("enabled", board_enabled)
        .value("on", board_on)
        .rows("rows", board_rows)
        .action("record", record);
    UiHarness::new(app)
}

/// `id` opened and its slide in finished.
fn opened_file(harness: &mut UiHarness, id: &str) {
    assert!(open(harness.world_mut(), id), "{id} opens");
    harness.step(12);
}

fn keyed(harness: &UiHarness, key: &str) -> WidgetId {
    harness
        .find_key(UiKey::new(key))
        .unwrap_or_else(|| panic!("no widget keyed {key}"))
}

fn heard(harness: &UiHarness) -> &[UiAction] {
    &harness.world().resource::<Board>().heard
}

/// Activates `widget` from the keyboard: focused, then `ui_accept`.
fn accept(harness: &mut UiHarness, widget: WidgetId) {
    assert!(harness.focus(widget));
    tap(harness, KeyCode::Enter);
}

#[test]
fn open_pushes_a_screen_file_by_id() {
    let pause = file(
        r#"{"kind": "heading", "text": "Paused", "centred": true},
           {"kind": "button", "key": "resume", "text": "Resume", "on": "pop"}"#,
    );
    let mut harness = data_harness(&[("pause", &pause)], true);
    opened_file(&mut harness, "pause");
    assert_eq!(stack(&harness).active_id(), Some("pause"));
    assert!(harness.shows("Paused"));
    let resume = keyed(&harness, "resume");
    assert_eq!(harness.tree().focused(), Some(resume), "its first control");
    assert!(!open(harness.world_mut(), "nowhere"));
    assert_eq!(
        refusal(harness.world(), "nowhere").as_deref(),
        Some("no registered screen or ui_screens file is named 'nowhere'")
    );
}

#[test]
fn a_data_screen_runs_each_action() {
    let menu = file(
        r#"{"kind": "button", "key": "named", "text": "Named", "on": {"action": "record"}},
           {"kind": "button", "key": "push", "text": "Push", "on": {"push": "other"}},
           {"kind": "button", "key": "pop", "text": "Pop", "on": "pop"}"#,
    );
    let other = file(r#"{"kind": "button", "key": "back", "text": "Back", "on": "pop"}"#);
    let mut harness = data_harness(&[("menu", &menu), ("other", &other)], true);
    opened_file(&mut harness, "menu");
    let named = keyed(&harness, "named");
    harness.click(named);
    harness.step(1);
    assert_eq!(
        heard(&harness),
        [UiAction {
            key: Some("named".into()),
            row: None,
            value: None
        }]
    );
    let push = keyed(&harness, "push");
    harness.click(push);
    harness.step(12);
    assert_eq!(stack(&harness).active_id(), Some("other"));
    let back = keyed(&harness, "back");
    harness.click(back);
    harness.step(12);
    assert_eq!(stack(&harness).active_id(), Some("menu"));
    let pop = keyed(&harness, "pop");
    harness.click(pop);
    harness.step(12);
    assert_eq!(stack(&harness).active_id(), None);
}

#[test]
fn each_control_runs_its_action_once() {
    let menu = file(
        r#"{"kind": "toggle_row", "key": "toggle", "caption": "Toggle", "on": {"action": "record"}},
           {"kind": "slider_row", "key": "slider", "caption": "Slider",
            "range": {"min": 0, "max": 1, "step": 0.25}, "on": {"action": "record"}},
           {"kind": "button", "key": "button", "text": "Button", "on": {"action": "record"}}"#,
    );
    let mut harness = data_harness(&[("menu", &menu)], true);
    opened_file(&mut harness, "menu");
    let values = |harness: &UiHarness| -> Vec<Option<UiValue>> {
        heard(harness).iter().map(|action| action.value).collect()
    };
    harness.click(keyed(&harness, "toggle"));
    harness.step(1);
    assert_eq!(
        values(&harness),
        [Some(UiValue::Bool(true))],
        "a toggle's click"
    );

    let slider = keyed(&harness, "slider");
    let rect = harness.rect(slider).expect("laid out");
    let y = f32::midpoint(rect.min.y, rect.max.y);
    let x = |share: f32| rect.min.x + (rect.max.x - rect.min.x) * share;
    harness.pointer_to(x(0.05), y);
    harness.pointer_down();
    harness.step(1);
    for share in [0.3, 0.6] {
        harness.pointer_to(x(share), y);
        harness.step(1);
    }
    harness.step(1);
    harness.pointer_up();
    harness.step(1);
    assert_eq!(values(&harness).len(), 2, "a drag over frames commits once");

    // A click where the value changes: one at the value it has commits
    // nothing.
    harness.pointer_to(x(0.97), y);
    harness.pointer_down();
    harness.pointer_up();
    harness.step(1);
    assert_eq!(values(&harness).len(), 3, "a slider's click");

    // At its most, a step down.
    assert!(harness.focus(slider));
    tap(&mut harness, KeyCode::ArrowLeft);
    assert_eq!(values(&harness).len(), 4, "a key's step");

    harness.click(keyed(&harness, "button"));
    harness.step(1);
    assert_eq!(values(&harness).len(), 5);
    assert_eq!(values(&harness)[4], None, "a button carries no value");
}

#[test]
fn a_dragged_setting_slider_commits_its_draft() {
    let menu = file(
        r#"{"kind": "slider_row", "key": "volume", "caption": "Volume",
            "range": {"min": 0, "max": 1, "step": 0.25}, "on": {"setting": "master_volume"}}"#,
    );
    let mut harness = data_harness(&[("menu", &menu)], true);
    opened_file(&mut harness, "menu");
    let slider = keyed(&harness, "volume");
    assert_eq!(
        harness.tree().slider_value(slider),
        Some(1.0),
        "the setting as it runs: the shipped volume"
    );
    let rect = harness.rect(slider).expect("laid out");
    let y = f32::midpoint(rect.min.y, rect.max.y);
    let middle = f32::midpoint(rect.min.x, rect.max.x);
    harness.pointer_to(rect.max.x - 1.0, y);
    harness.pointer_down();
    harness.step(1);
    harness.pointer_to(middle, y);
    harness.step(1);
    // Off the slider: the draft holds, every frame's refresh leaves it.
    harness.pointer_to(middle, y + 200.0);
    harness.step(1);
    harness.step(1);
    assert_eq!(harness.tree().slider_value(slider), Some(0.5));
    assert_eq!(harness.world().resource::<Settings>().master_volume(), None);
    harness.pointer_up();
    harness.step(1);
    assert_eq!(
        harness.world().resource::<Settings>().master_volume(),
        Some(0.5)
    );
    harness.step(1);
    assert_eq!(harness.tree().slider_value(slider), Some(0.5));
}

#[test]
fn a_setting_action_writes_and_applies_settings() {
    let menu = file(
        r#"{"kind": "toggle_row", "key": "vsync", "caption": "VSync", "on": {"setting": "vsync"}},
           {"kind": "slider_row", "key": "scale", "caption": "UI scale",
            "range": {"min": 0.5, "max": 2, "step": 0.25}, "on": {"setting": "ui_scale"}}"#,
    );
    let mut harness = data_harness(&[("menu", &menu)], true);
    opened_file(&mut harness, "menu");
    let vsync = keyed(&harness, "vsync");
    let was = harness.tree().toggled(vsync).expect("a toggle");
    harness.click(vsync);
    harness.step(1);
    assert_eq!(harness.world().resource::<Settings>().vsync(), Some(!was));
    let scale = keyed(&harness, "scale");
    assert_eq!(harness.tree().slider_value(scale), Some(1.0));
    assert!(harness.focus(scale));
    tap(&mut harness, KeyCode::ArrowRight);
    let world = harness.world();
    assert_eq!(world.resource::<Settings>().ui_scale(), Some(1.25));
    assert_eq!(
        world.resource::<tungsten_core::ScaleFactor>().ui_scale(),
        1.25,
        "applied"
    );
}

#[test]
fn a_named_action_gets_its_key_and_value() {
    let menu = file(
        r#"{"kind": "toggle_row", "key": "music", "caption": "Music", "bind": {"value": "on"},
            "on": {"action": "record"}},
           {"kind": "button", "text": "Unkeyed", "on": {"action": "record"}}"#,
    );
    let mut harness = data_harness(&[("menu", &menu)], true);
    opened_file(&mut harness, "menu");
    harness.click(keyed(&harness, "music"));
    harness.step(1);
    let unkeyed = harness.find("Unkeyed").expect("a button");
    accept(&mut harness, unkeyed);
    assert_eq!(
        heard(&harness),
        [
            UiAction {
                key: Some("music".into()),
                row: None,
                value: Some(UiValue::Bool(true))
            },
            UiAction {
                key: None,
                row: None,
                value: None
            }
        ]
    );
}

#[test]
fn a_row_action_names_its_row() {
    let menu = file(
        r#"{"kind": "repeat", "key": "slots", "rows": "rows",
            "row": {"kind": "button", "text": "Slot", "on": {"action": "record"},
                    "children": [{"kind": "button", "key": "delete", "text": "x", "on": {"action": "record"}}]}}"#,
    );
    let mut harness = data_harness(&[("menu", &menu)], true);
    opened_file(&mut harness, "menu");
    let row_b = keyed(&harness, "slots/b");
    accept(&mut harness, row_b);
    let delete_a = keyed(&harness, "slots/a/delete");
    accept(&mut harness, delete_a);
    assert_eq!(
        heard(&harness),
        [
            UiAction {
                key: Some("slots".into()),
                row: Some("b".into()),
                value: None
            },
            UiAction {
                key: Some("delete".into()),
                row: Some("a".into()),
                value: None
            }
        ]
    );
}

#[test]
fn open_works_without_the_plugin() {
    let pause = file(r#"{"kind": "button", "key": "resume", "text": "Resume", "on": "pop"}"#);
    let mut harness = data_harness(&[("pause", &pause)], false);
    assert!(!harness.world().has_resource::<Screens>());
    opened_file(&mut harness, "pause");
    let world = harness.world();
    assert!(world.has_resource::<Screens>());
    assert!(world.has_resource::<ScreenRegistry>());
    assert!(world.has_resource::<Toasts>());
    assert_eq!(stack(&harness).active_id(), Some("pause"));
    assert!(harness.shows("Resume"));
}

#[test]
fn open_refuses_a_file_off_the_popup_layer_or_not_modal() {
    let root = |layer: &str, modal: bool| {
        format!(
            r#"{{"version": 1, "root": {{"kind": "root", "layer": "{layer}", "modal": {modal},
                 "children": [{{"kind": "label", "text": "x"}}]}}}}"#
        )
    };
    let (hud, panel, popup) = (
        root("game", true),
        root("game_popup", false),
        root("game_popup", true),
    );
    let mut harness = data_harness(&[("hud", &hud), ("panel", &panel), ("popup", &popup)], true);
    for id in ["hud", "panel"] {
        assert!(!open(harness.world_mut(), id), "{id}");
        assert_eq!(
            refusal(harness.world(), id).as_deref(),
            Some(format!("ui screen '{id}': its root is not modal on the popup layer").as_str())
        );
    }
    harness.step(1);
    assert_eq!(stack(&harness).active_id(), None);
    opened_file(&mut harness, "popup");
    assert_eq!(stack(&harness).active_id(), Some("popup"));
}

#[test]
fn a_data_screen_pops_on_cancel_and_refocuses_its_opener() {
    let menu = file(
        r#"{"kind": "button", "key": "first", "text": "First"},
           {"kind": "button", "key": "open", "text": "Open", "on": {"push": "child"}}"#,
    );
    let child = file(r#"{"kind": "button", "key": "x", "text": "X"}"#);
    let mut harness = data_harness(&[("menu", &menu), ("child", &child)], true);
    opened_file(&mut harness, "menu");
    let opener = keyed(&harness, "open");
    accept(&mut harness, opener);
    harness.step(12);
    assert_eq!(stack(&harness).active_id(), Some("child"));
    tap(&mut harness, KeyCode::Escape);
    harness.step(12);
    assert_eq!(stack(&harness).active_id(), Some("menu"));
    assert_eq!(harness.tree().focused(), Some(opener), "back on its opener");
}

/// The idle data-screen guard (redo 7.20): a data screen whose bindings
/// refresh every frame lays out nothing while their values hold; one
/// binding's new value lays out its root once.
#[test]
fn an_idle_data_screen_lays_out_no_root() {
    let menu = file(
        r#"{"kind": "label", "key": "label", "bind": {"text": "text", "visible": "shown", "enabled": "enabled"}},
           {"kind": "toggle_row", "caption": "On", "bind": {"value": "on", "enabled": "enabled"}},
           {"kind": "button", "text": "Go", "bind": {"visible": "shown"}}"#,
    );
    let mut harness = data_harness(&[("menu", &menu)], true);
    harness.world_mut().resource_mut::<Board>().text = "hello".into();
    opened_file(&mut harness, "menu");
    let before = harness.layout_totals();
    harness.step(60);
    assert_eq!(
        harness.layout_totals(),
        before,
        "60 refreshed frames lay nothing out"
    );
    harness.world_mut().resource_mut::<Board>().text = "changed".into();
    harness.step(1);
    assert_eq!(
        harness.layout_totals().roots_laid_out,
        before.roots_laid_out + 1,
        "a binding's new value lays out its root"
    );
    harness.step(10);
    assert_eq!(
        harness.layout_totals().roots_laid_out,
        before.roots_laid_out + 1,
        "once"
    );
}
