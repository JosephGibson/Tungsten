//! A modal question with two answers, stacked under it in a menu-wide
//! card: the safe one, the second, focused and picked by `ui_cancel`, by
//! an optional countdown on the real clock when it ends, and by the game's
//! exit. An answer runs its callback, then the dialog pops, unless the
//! callback changed the run through `goto`, which takes the dialog too.

use tungsten::core::text::TextAlign;
use tungsten::core::ui::{UiTree, WidgetId};
use tungsten::core::{Time, Timer, World};
use tungsten::{Exiting, GameState, StateContext, StateId};

use super::{cancelled, clicked, enter, flow_pending, idle, pop, theme, widgets};

pub const CONFIRM: StateId = "confirm";

type Answer = Box<dyn FnOnce(&mut World)>;

/// The confirm dialog; push it with [`ask`].
pub struct ConfirmState {
    question: String,
    yes: &'static str,
    no: &'static str,
    on_yes: Option<Answer>,
    on_no: Option<Answer>,
    /// The real-clock timer that picks the safe answer when it ends.
    countdown: Option<Timer>,
    ui: Option<[WidgetId; 4]>,
}

impl ConfirmState {
    /// Asks `question` with the answers `yes` and `no`, the safe one.
    #[must_use]
    pub fn new(question: &str, yes: &'static str, no: &'static str) -> Self {
        Self {
            question: question.to_owned(),
            yes,
            no,
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

    /// Runs `answer` on `no`, `ui_cancel` or the countdown's end.
    #[must_use]
    pub fn on_no(mut self, answer: impl FnOnce(&mut World) + 'static) -> Self {
        self.on_no = Some(Box::new(answer));
        self
    }

    /// Picks `no` after `secs` real seconds.
    #[must_use]
    pub fn countdown(mut self, secs: f32) -> Self {
        self.countdown = Some(Timer::once(secs));
        self
    }

    /// Runs the chosen answer's callback and drops the other's, so the first
    /// answer is final, even one the exit gives while the dialog fades out.
    fn answer(&mut self, world: &mut World, yes: bool) {
        let answers = [self.on_no.take(), self.on_yes.take()];
        if let Some(answer) = answers.into_iter().nth(usize::from(yes)).flatten() {
            answer(world);
        }
        if !flow_pending(world) {
            pop(world);
        }
    }
}

/// Pushes `confirm` over the screen on top, which stays drawn under its
/// scrim.
pub fn ask(world: &mut World, confirm: ConfirmState) {
    super::push(world, confirm);
}

/// The countdown's line: its whole seconds left.
fn count_text(timer: &Timer) -> String {
    format!("Reverting in {} s", timer.remaining().ceil())
}

impl GameState for ConfirmState {
    fn id(&self) -> StateId {
        CONFIRM
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        let tree = ctx.world.resource_mut::<UiTree>();
        let (root, card) = widgets::screen(tree, theme::MENU_WIDTH);
        let question = tree.label(card, self.question.as_str());
        let count = widgets::note(tree, card, "");
        for label in [question, count] {
            widgets::restyle(tree, label, |text| text.align = Some(TextAlign::Center));
        }
        widgets::show(tree, count, self.countdown.is_some());
        if let Some(timer) = &self.countdown {
            tree.set_plain_text(count, &count_text(timer));
        }
        let yes = widgets::button(tree, card, self.yes);
        let no = widgets::button(tree, card, self.no);
        tree.focus(no);
        enter(ctx.world, root);
        self.ui = Some([root, yes, no, count]);
    }

    fn on_exit(&mut self, _ctx: &mut StateContext) {}

    fn update(&mut self, world: &mut World) {
        let Some([root, yes, no, count]) = self.ui else {
            return;
        };
        // The run's last frame takes the safe answer, so an unkept display
        // reverts before the exit saves the settings.
        if world.has_resource::<Exiting>() {
            return self.answer(world, false);
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
                return self.answer(world, false);
            }
        }
        match clicked(world, &[yes, no]) {
            Some(0) => self.answer(world, true),
            Some(_) => self.answer(world, false),
            None if cancelled(world, root) => self.answer(world, false),
            None => {}
        }
    }
}
