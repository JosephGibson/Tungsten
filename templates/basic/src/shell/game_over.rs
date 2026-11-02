//! Game over, pushed over gameplay when the player dies: across the screen
//! a dark band holding "YOU DIED" in spaced crimson serif capitals, which
//! fades in over [`theme::DIED_SECS`] while the world pixelates and dims
//! under a near-black crimson (the shell's post tail, [`passes`]), with a
//! sting and music of its own from the start. The entrance is a screen
//! change, so nothing acts during it; then Retry, Load and Quit to Title
//! fade in under the band, Retry focused. Quit to Title asks nothing, since
//! a dead run saves nothing. Gameplay holds the game clock while covered.
//! Retry loads the run's last good save at the run's difficulty, or starts
//! the run fresh when it has none.

use tungsten::core::post::PostPass;
use tungsten::core::ui::{UiKey, UiTree, Visibility, WidgetId, widgets};
use tungsten::core::{AudioCommands, Easing, Voice, World};
use tungsten::{GameState, RootCover, StateContext, StateId, TransitionEffect};

use super::saves::{Mode, SavesState};
use super::title::TitleState;
use super::{
    Change, Menu, RunStart, Shell, SlotState, goto, idle, play, push, read_slots, run, stack,
    start_run, theme,
};

pub const GAME_OVER: StateId = "game_over";
const RETRY: UiKey = UiKey::new("retry");
const LOAD: UiKey = UiKey::new("load");
const QUIT_TITLE: UiKey = UiKey::new("quit_title");
/// The panel holding the buttons, in `assets/ui/screens/game_over.json`.
const CARD: UiKey = UiKey::new("card");

/// The game-over screen and its music.
#[derive(Default)]
pub struct GameOverState {
    menu: Option<Menu>,
    music: Option<Voice>,
}

/// Game over's post passes `secs` into it: the world pixelated toward
/// [`theme::DIED_BLOCK_PX`] and dimmed toward [`theme::DIED_DIM`] under
/// [`theme::DIED_FADE`], eased out over the entrance.
#[must_use]
pub fn passes(secs: f32) -> Vec<PostPass> {
    let cover = Easing::QuadOut.apply((secs / theme::DIED_SECS).min(1.0));
    let block = theme::DIED_BLOCK_PX;
    let pixelate = TransitionEffect::Pixelate {
        max_block_px: block,
    };
    let fade = TransitionEffect::Fade {
        color: theme::DIED_FADE,
    };
    vec![
        pixelate.pass_at(cover),
        fade.pass_at(theme::DIED_DIM * cover),
    ]
}

/// Game over's edits its file cannot say: twice `spacing.lg` between the
/// band and the buttons, and during the entrance the band fading in and the
/// buttons hidden until [`reveal`]; again after a reload rebuilds them.
fn edit(tree: &mut UiTree, root: WidgetId, entering: bool) {
    let gap = 2.0 * tree.theme().tokens.spacing.lg;
    tree.edit_layout(root, |style| style.gap = gap);
    let band = tree.children(root).first().copied();
    if let (true, Some(band), Some(card)) = (entering, band, tree.find_key(root, CARD)) {
        tree.set_visibility(card, Visibility::Hidden);
        theme::fade_in(tree, band, theme::DIED_SECS);
    }
}

/// The entrance ended: game over's buttons, its `root`'s card, show and
/// fade in, the first that can take focus focused.
pub(super) fn reveal(world: &mut World, root: WidgetId) {
    let tree = world.resource_mut::<UiTree>();
    if let Some(card) = tree.find_key(root, CARD) {
        tree.set_visibility(card, Visibility::Visible);
        theme::fade_in(tree, card, theme::DIED_BUTTONS_SECS);
        let buttons = tree.children(card).to_vec();
        widgets::focus_first(tree, buttons);
    }
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
        let world = &mut *ctx.world;
        // The band across a clear modal root, the buttons under it.
        let menu = Menu::build(world, "game_over");
        edit(world.resource_mut::<UiTree>(), menu.root, true);
        stack(world).own_root(menu.root, RootCover::Show);
        let shell = world.resource_mut::<Shell>();
        (shell.change, shell.died) = (Some(Change::Entrance(0.0, menu.root)), Some(0.0));
        let [sting, music] = theme::GAME_OVER_SOUNDS;
        play(world, sting, 0.0);
        self.music = play(world, music, theme::GAME_OVER_FADE);
        self.menu = Some(menu);
    }

    fn on_exit(&mut self, ctx: &mut StateContext) {
        ctx.world.resource_mut::<Shell>().died = None;
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
        if let Some(menu) = self.menu.as_mut()
            && menu.revised(world)
        {
            let entering = matches!(world.resource::<Shell>().change, Some(Change::Entrance(..)));
            edit(world.resource_mut::<UiTree>(), menu.root, entering);
        }
        let Some(menu) = self.menu.as_mut().filter(|_| idle(world)) else {
            return;
        };
        match menu.chosen(world) {
            Some(RETRY) => retry(world),
            Some(LOAD) => push(world, SavesState::new(Mode::Load)),
            Some(QUIT_TITLE) => goto(world, TitleState::default()),
            _ => {}
        }
    }
}
