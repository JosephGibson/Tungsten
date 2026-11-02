//! A modal question with two answers (redo 7.17): the safe one, the
//! second, focused and taken by a cancel, by an optional countdown on the
//! real clock when it ends, and by the game's exit. The first answer is
//! final: it runs its callback, then the dialog slides out and pops, unless
//! the callback changed the stack, which takes the dialog too.

use tungsten_core::text::TextAlign;
use tungsten_core::ui::{UiEvent, UiKey, UiTree, WidgetId, widgets};
use tungsten_core::{EventQueue, Time, Timer, World};

use super::{ModalScreen, idle, pop};
use crate::app::Exiting;
use crate::state::{GameState, StateContext, StateId};

/// The confirm dialog's state ID.
pub const CONFIRM: StateId = "confirm";

/// The keys of the two answers' buttons.
const YES: UiKey = UiKey::new("yes");
const NO: UiKey = UiKey::new("no");

type Answer = Box<dyn FnOnce(&mut World)>;

/// The confirm dialog; push it with [`ask`].
pub struct Confirm {
    question: String,
    yes: String,
    no: String,
    on_yes: Option<Answer>,
    on_no: Option<Answer>,
    /// The real-clock timer that takes the safe answer when it ends.
    countdown: Option<Timer>,
    /// The screen and its countdown line, once entered.
    ui: Option<(ModalScreen, WidgetId)>,
}

impl Confirm {
    /// Asks `question` with the answers `yes` and `no`, the safe one,
    /// their buttons keyed `yes` and `no`.
    #[must_use]
    pub fn new(question: &str, yes: &str, no: &str) -> Self {
        Self {
            question: question.to_owned(),
            yes: yes.to_owned(),
            no: no.to_owned(),
            on_yes: None,
            on_no: None,
            countdown: None,
            ui: None,
        }
    }

    /// Runs `answer` on `yes`.
    #[must_use]
    pub fn on_yes(mut self, answer: impl FnOnce(&mut World) + 'static) -> Self {
        self.on_yes = Some(Box::new(answer));
        self
    }

    /// Runs `answer` on `no`, a cancel, the countdown's end or the exit.
    #[must_use]
    pub fn on_no(mut self, answer: impl FnOnce(&mut World) + 'static) -> Self {
        self.on_no = Some(Box::new(answer));
        self
    }

    /// Takes `no` after `secs` real seconds, counting them down on the
    /// dialog.
    #[must_use]
    pub fn countdown(mut self, secs: f32) -> Self {
        self.countdown = Some(Timer::once(secs));
        self
    }

    /// Runs the chosen answer's callback and drops the other's, so the first
    /// answer is final; then the dialog pops unless the callback changed the
    /// stack.
    fn answer(&mut self, world: &mut World, yes: bool) {
        let answers = [self.on_no.take(), self.on_yes.take()];
        let chosen = answers.into_iter().nth(usize::from(yes)).flatten();
        let answered = chosen.is_some();
        if let Some(answer) = chosen {
            answer(world);
        }
        if answered && idle(world) {
            pop(world);
        }
    }
}

/// Asks `confirm` over the screen on top, which stays drawn under its
/// scrim; `false` unless the screens are idle.
pub fn ask(world: &mut World, confirm: Confirm) -> bool {
    super::push(world, confirm)
}

/// The countdown's line: its whole seconds left.
fn count_text(timer: &Timer) -> String {
    format!("{} s", timer.remaining().ceil())
}

impl GameState for Confirm {
    fn id(&self) -> StateId {
        CONFIRM
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        let width = widgets::size(ctx.world.resource::<UiTree>(), "menu_width");
        let screen = ModalScreen::open(ctx.world, width);
        let tree = ctx.world.resource_mut::<UiTree>();
        let question = tree.label(screen.card, self.question.as_str());
        let count = widgets::note(tree, screen.card, "");
        for label in [question, count] {
            widgets::restyle(tree, label, |text| text.align = Some(TextAlign::Center));
        }
        widgets::show(tree, count, self.countdown.is_some());
        if let Some(timer) = &self.countdown {
            tree.set_plain_text(count, &count_text(timer));
        }
        let yes = widgets::button(tree, screen.card, &self.yes);
        tree.set_key(yes, "yes");
        let no = widgets::button(tree, screen.card, &self.no);
        tree.set_key(no, "no");
        tree.focus(no);
        self.ui = Some((screen, count));
    }

    fn on_exit(&mut self, _ctx: &mut StateContext) {}

    fn update(&mut self, world: &mut World) {
        let Some((mut screen, count)) = self.ui else {
            return;
        };
        // The run's last frame takes the safe answer, so a change the
        // question guards reverts before the exit saves.
        if world.has_resource::<Exiting>() {
            return self.answer(world, false);
        }
        let cancelled = idle(world)
            && world.resource::<EventQueue<UiEvent>>().iter_current().any(
                |event| matches!(event, UiEvent::Cancel { scope, .. } if *scope == screen.root),
            );
        if cancelled {
            return self.answer(world, false);
        }
        let key = screen.update(world);
        self.ui = Some((screen, count));
        match key {
            Some(YES) => return self.answer(world, true),
            Some(NO) => return self.answer(world, false),
            _ => {}
        }
        if !idle(world) {
            return;
        }
        if let Some(timer) = self.countdown.as_mut() {
            let ended = timer.tick(world.resource::<Time>().real_delta()) > 0;
            let text = count_text(timer);
            world.resource_mut::<UiTree>().set_plain_text(count, &text);
            if ended {
                self.countdown = None;
                self.answer(world, false);
            }
        }
    }
}
