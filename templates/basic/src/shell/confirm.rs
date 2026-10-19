//! A modal question with two answers: the safe one, the second, focused and
//! picked by `ui_cancel`; an optional countdown on the real clock picks it
//! when it ends. An answer runs its callback, then the dialog pops, unless
//! the callback changed the run through `goto`, which takes the dialog too.

use tungsten::core::ui::{Justify, UiTree, WidgetId};
use tungsten::core::{Time, World};
use tungsten::{GameState, RootCover, StateContext, StateId};

use super::{cancelled, clicked, flow_pending, idle, pop, stack, theme, widgets};

pub const CONFIRM: StateId = "confirm";

type Answer = Box<dyn FnOnce(&mut World)>;

/// The confirm dialog; push it with [`ask`].
pub struct ConfirmState {
    question: String,
    yes: &'static str,
    no: &'static str,
    on_yes: Option<Answer>,
    on_no: Option<Answer>,
    /// Real seconds left before the safe answer, when counting down.
    countdown: Option<f32>,
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
        self.countdown = Some(secs);
        self
    }

    fn answer(&mut self, world: &mut World, yes: bool) {
        let answer = if yes {
            self.on_yes.take()
        } else {
            self.on_no.take()
        };
        if let Some(answer) = answer {
            answer(world);
        }
        if !flow_pending(world) {
            pop(world);
        }
    }
}

/// Pushes `confirm` over the screen on top, behind a fade.
pub fn ask(world: &mut World, confirm: ConfirmState) {
    super::push(world, confirm);
}

fn count_text(secs: f32) -> String {
    format!("Reverting in {} s", secs.ceil().max(0.0))
}

impl GameState for ConfirmState {
    fn id(&self) -> StateId {
        CONFIRM
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        let tree = ctx.world.resource_mut::<UiTree>();
        let root = widgets::screen(tree, theme::MENU_WIDTH + 80.0);
        tree.label(root, self.question.as_str());
        let count = widgets::note(tree, root, "");
        widgets::show(tree, count, self.countdown.is_some());
        if let Some(secs) = self.countdown {
            tree.set_plain_text(count, &count_text(secs));
        }
        let row = widgets::row(tree, root);
        let mut style = tree.layout_style(row).cloned().unwrap_or_default();
        style.justify_content = Justify::End;
        tree.set_layout_style(row, style);
        let yes = widgets::button(tree, row, self.yes);
        let no = widgets::button(tree, row, self.no);
        tree.focus(no);
        theme::slide_in(tree, root);
        stack(ctx.world).own_root(root, RootCover::Hide);
        self.ui = Some([root, yes, no, count]);
    }

    fn on_exit(&mut self, _ctx: &mut StateContext) {}

    fn update(&mut self, world: &mut World) {
        let Some([root, yes, no, count]) = self.ui else {
            return;
        };
        if !idle(world) {
            return;
        }
        if let Some(left) = self.countdown.as_mut() {
            *left -= world.resource::<Time>().real_delta();
            let text = count_text(*left);
            world.resource_mut::<UiTree>().set_plain_text(count, &text);
            if *left <= 0.0 {
                self.countdown = None;
                self.answer(world, false);
                return;
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
