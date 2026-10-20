//! Pause, pushed over gameplay: Resume, Save, Load, Gameplay Options,
//! Settings, Quit to Title and Quit Game. Gameplay holds the game clock while
//! covered; pause autosaves on its first frame that runs no fixed step, since
//! the frame that pushed it still ran the steps it owed. `ui_cancel`,
//! Resume or `state_pause` closes it, once a frame.

use tungsten::core::ui::UiTree;
use tungsten::core::{Time, World};
use tungsten::{GameState, StateContext, StateId};

use super::saves::{Mode, SavesState};
use super::settings::{GAMEPLAY_TAB, SettingsState};
use super::{
    Menu, autosave, cancelled, idle, pop, pressed, push, quit_game, quit_to_title, saving_on,
};

pub const PAUSE: StateId = "pause";

/// The pause screen.
#[derive(Default)]
pub struct PauseState {
    saved: bool,
    menu: Option<Menu>,
}

impl GameState for PauseState {
    fn id(&self) -> StateId {
        PAUSE
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        let captions = [
            "Resume",
            "Save",
            "Load",
            "Gameplay Options",
            "Settings",
            "Quit to Title",
            "Quit Game",
        ];
        let menu = Menu::new(ctx.world, "Paused", &captions);
        let saving = saving_on(ctx.world);
        let tree = ctx.world.resource_mut::<UiTree>();
        tree.set_enabled(menu.buttons[1], saving);
        tree.set_enabled(menu.buttons[2], saving);
        self.menu = Some(menu);
    }

    fn on_exit(&mut self, _ctx: &mut StateContext) {}

    fn on_resume(&mut self, ctx: &mut StateContext) {
        if let Some(menu) = &self.menu {
            menu.focus(ctx.world);
        }
    }

    fn update(&mut self, world: &mut World) {
        if !self.saved && world.resource::<Time>().fixed_steps_this_frame() == 0 {
            self.saved = true;
            autosave(world);
        }
        let Some(menu) = self.menu.as_mut().filter(|_| idle(world)) else {
            return;
        };
        match menu.pick(world) {
            Some(1) => push(world, SavesState::new(Mode::Save)),
            Some(2) => push(world, SavesState::new(Mode::Load)),
            Some(3) => push(world, SettingsState::new(GAMEPLAY_TAB)),
            Some(4) => push(world, SettingsState::new(0)),
            Some(5) => quit_to_title(world),
            Some(6) => quit_game(world),
            Some(_) => pop(world),
            None if cancelled(world, menu.root) || pressed(world, "state_pause", false) => {
                pop(world);
            }
            None => {}
        }
    }
}
