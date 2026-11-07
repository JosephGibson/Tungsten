//! Pause, pushed over gameplay: Resume, Save, Load, Settings, Quit to Title
//! and Quit Game; the gameplay options sit in Settings' Gameplay tab.
//! Gameplay holds the game clock while covered; pause autosaves on its
//! first frame that runs no fixed step, since the frame that pushed it
//! still ran the steps it owed. `ui_cancel`, Resume or `state_pause` closes
//! it, once a frame.

use tungsten::core::ui::UiKey;
use tungsten::core::{Time, World};
use tungsten::{GameState, StateContext, StateId, StateMusic};

use super::saves::{Mode, SavesState};
use super::settings::SettingsState;
use super::{Menu, autosave, cancelled, idle, pop, pressed, push, quit_game, quit_to_title, theme};

pub const PAUSE: StateId = "pause";
const SAVE: UiKey = UiKey::new("save");
const LOAD: UiKey = UiKey::new("load");
const SETTINGS: UiKey = UiKey::new("settings");
const QUIT_TITLE: UiKey = UiKey::new("quit_title");
const QUIT_GAME: UiKey = UiKey::new("quit_game");

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
        self.menu = Some(Menu::new(ctx.world, "pause"));
    }

    fn on_exit(&mut self, _ctx: &mut StateContext) {}

    fn on_resume(&mut self, ctx: &mut StateContext) {
        if let Some(menu) = &self.menu {
            menu.focus(ctx.world);
        }
    }

    /// Pause's loop, the run's song at 0 under it.
    fn music(&self) -> Option<StateMusic> {
        Some(StateMusic::new(theme::PAUSE_MUSIC, theme::PAUSE_FADE))
    }

    fn update(&mut self, world: &mut World) {
        if !self.saved && world.resource::<Time>().fixed_steps_this_frame() == 0 {
            self.saved = true;
            autosave(world);
        }
        let Some(menu) = self.menu.as_mut().filter(|_| idle(world)) else {
            return;
        };
        match menu.chosen(world) {
            Some(SAVE) => push(world, SavesState::new(Mode::Save)),
            Some(LOAD) => push(world, SavesState::new(Mode::Load)),
            Some(SETTINGS) => push(world, SettingsState::new(0)),
            Some(QUIT_TITLE) => quit_to_title(world),
            Some(QUIT_GAME) => quit_game(world),
            Some(_) => pop(world),
            None if cancelled(world, menu.root) || pressed(world, "state_pause", false) => {
                pop(world);
            }
            None => {}
        }
    }
}
