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
use tungsten::core::ui::{
    Align, Justify, LayoutStyle, Sizing, UiLayer, UiTree, Visibility, WidgetId,
};
use tungsten::core::{AudioCommands, Easing, Voice, World};
use tungsten::{GameState, RootCover, StateContext, StateId, TransitionEffect};

use super::saves::{Mode, SavesState};
use super::title::TitleState;
use super::{
    Change, Menu, RunStart, Shell, SlotState, goto, idle, play, push, read_slots, run, saving_on,
    stack, start_run, theme, widgets,
};

pub const GAME_OVER: StateId = "game_over";

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

/// The entrance ended: game over's buttons, `card`, show and fade in, the
/// first that can take focus focused.
pub(super) fn reveal(world: &mut World, card: WidgetId) {
    let tree = world.resource_mut::<UiTree>();
    if tree.contains(card) {
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
        let saving = saving_on(world);
        let tree = world.resource_mut::<UiTree>();
        // A clear modal root: the band across it, the buttons under it.
        let full = Sizing::Percent(100.0);
        let mut style = LayoutStyle::column()
            .with_size(full, full)
            .with_gap(2.0 * theme::PADDING);
        (style.align_items, style.justify_content) = (Align::Center, Justify::Center);
        let root = tree.add_root(UiLayer::GamePopup, style);
        tree.set_modal(root, true);
        widgets::fill(tree, root, [0; 4]);
        let band = LayoutStyle::column().with_size(full, Sizing::Auto);
        let band = tree.panel(root, band.with_padding(theme::PADDING));
        widgets::fill(tree, band, theme::DIED_BAND);
        let heading = widgets::display(tree, band, "YOU DIED", theme::DIED);
        widgets::restyle(tree, heading, |text| {
            text.color = Some(theme::DIED_RED);
            text.letter_spacing = Some(theme::DIED_SPACING);
        });
        let card = LayoutStyle::column().with_gap(theme::GAP);
        let width = Sizing::Px(theme::MENU_WIDTH);
        let card = tree.panel(root, card.with_size(width, Sizing::Auto));
        widgets::fill(tree, card, [0; 4]);
        let captions = ["Retry", "Load", "Quit to Title"];
        let buttons: Vec<_> = captions
            .iter()
            .map(|caption| widgets::button(tree, card, caption))
            .collect();
        tree.set_enabled(buttons[1], saving);
        tree.set_visibility(card, Visibility::Hidden);
        theme::fade_in(tree, band, theme::DIED_SECS);
        stack(world).own_root(root, RootCover::Show);
        widgets::capped(world, heading, theme::DIED, theme::DIED_CAP);
        let shell = world.resource_mut::<Shell>();
        (shell.change, shell.died) = (Some(Change::Entrance(0.0, card)), Some(0.0));
        let [sting, music] = theme::GAME_OVER_SOUNDS;
        play(world, sting, 0.0);
        self.music = play(world, music, theme::GAME_OVER_FADE);
        let opened = None;
        self.menu = Some(Menu {
            root,
            heading,
            buttons,
            opened,
        });
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
