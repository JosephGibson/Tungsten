use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec2;
use tungsten_core::ui::{UiKey, UiTree, widgets};
use tungsten_core::{Config, KeyCode, World};

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
