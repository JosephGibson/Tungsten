//! Game over, pushed over gameplay when the player dies: "You died" in red
//! falling into place as the world sinks into a red dusk, a sting and music
//! of its own, then Retry, Load and Quit to Title, which asks nothing,
//! since a dead run saves nothing. Gameplay holds the game clock while
//! covered. Retry loads the run's last good save at the run's difficulty,
//! or starts the run fresh when it has none.

use tungsten::core::ui::UiTree;
use tungsten::core::{AudioCommands, Voice, World};
use tungsten::{GameState, StateContext, StateId};

use super::saves::{Mode, SavesState};
use super::title::TitleState;
use super::{
    Menu, RunStart, SlotState, goto, idle, play, push, read_slots, run, saving_on, start_run,
    theme, widgets,
};

pub const GAME_OVER: StateId = "game_over";

/// The game-over screen and its music.
#[derive(Default)]
pub struct GameOverState {
    menu: Option<Menu>,
    music: Option<Voice>,
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
        let menu = Menu::new(ctx.world, "You died", &["Retry", "Load", "Quit to Title"]);
        let saving = saving_on(ctx.world);
        let tree = ctx.world.resource_mut::<UiTree>();
        tree.set_enabled(menu.buttons[1], saving);
        widgets::danger(tree, menu.heading);
        theme::died(tree, menu.root, menu.heading);
        widgets::capped(ctx.world, menu.heading, theme::DIED, theme::DIED_CAP);
        let [sting, music] = theme::GAME_OVER_SOUNDS;
        play(ctx.world, sting, 0.0);
        self.music = play(ctx.world, music, theme::GAME_OVER_FADE);
        self.menu = Some(menu);
    }

    fn on_exit(&mut self, ctx: &mut StateContext) {
        if let Some(music) = self.music.take() {
            let audio = ctx.world.resource_mut::<AudioCommands>();
            audio.stop_voice(music, theme::GAME_OVER_FADE);
        }
    }

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
