//! Game over, pushed over gameplay when the player dies: Retry, Load and
//! Quit to Title, which asks nothing, since a dead run saves nothing.
//! Gameplay holds the game clock while covered. Retry loads the run's last
//! good save at the run's difficulty, or starts the run fresh when it has
//! none.

use tungsten::core::World;
use tungsten::core::ui::UiTree;
use tungsten::{GameState, StateContext, StateId};

use super::saves::{Mode, SavesState};
use super::title::TitleState;
use super::{Menu, RunStart, SlotState, goto, idle, push, read_slots, run, saving_on, start_run};

pub const GAME_OVER: StateId = "game_over";

/// The game-over screen.
#[derive(Default)]
pub struct GameOverState {
    menu: Option<Menu>,
}

/// Starts the run again: from the slot it last loaded or saved, when that
/// still reads, else fresh; at the run's difficulty either way.
pub fn retry(world: &mut World) {
    let Some((difficulty, from)) = run(world).map(|run| (run.difficulty, run.retry_from)) else {
        return;
    };
    let (slots, _) = read_slots(world);
    let ready = slots.into_iter().find_map(|(slot, state)| match state {
        SlotState::Ready(save, _) if from == Some(slot) => Some((slot, *save)),
        _ => None,
    });
    let (from, save) = ready.map_or((None, None), |(slot, save)| (Some(slot), Some(save)));
    start_run(world, RunStart { difficulty, save }, from);
}

impl GameState for GameOverState {
    fn id(&self) -> StateId {
        GAME_OVER
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        let menu = Menu::new(ctx.world, "Game over", &["Retry", "Load", "Quit to Title"]);
        let saving = saving_on(ctx.world);
        ctx.world
            .resource_mut::<UiTree>()
            .set_enabled(menu.buttons[1], saving);
        self.menu = Some(menu);
    }

    fn on_exit(&mut self, _ctx: &mut StateContext) {}

    fn on_resume(&mut self, ctx: &mut StateContext) {
        if let Some(menu) = &self.menu {
            menu.focus(ctx.world);
        }
    }

    fn update(&mut self, world: &mut World) {
        let Some(menu) = self.menu.as_mut().filter(|_| idle(world)) else {
            return;
        };
        match menu.pick(world) {
            Some(0) => retry(world),
            Some(1) => push(world, SavesState::new(Mode::Load)),
            Some(2) => goto(world, TitleState::default()),
            _ => {}
        }
    }
}
