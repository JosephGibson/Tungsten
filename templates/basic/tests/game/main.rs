//! The game on the engine's headless harness, laid out after each frame: no
//! window or renderer, the frames stepped one call at a time. `sample` holds
//! the sample's gameplay, `shell` the shell's screens and run, `saves` the
//! save seam and its screen, `settings` the tabbed settings, `keys` the
//! keybindings, `fit` every screen at six windows and the scale extremes,
//! and `visual` the title's pixels.

mod fit;
mod keys;
mod sample;
mod saves;
mod script;
mod settings;
mod shell;
mod visual;

use std::path::{Path, PathBuf};

use tungsten::core::ui::{Visibility, WidgetId};
use tungsten::core::{
    AudioCommand, AudioHandle, Config, Entity, Position, SoundRegistry, Transform, UserFolder,
    Velocity, Voice, With,
};
use tungsten::kit::Health;
use tungsten::testing::UiHarness;
use tungsten::{App, StateId};
use tungsten_template_basic::game;
use tungsten_template_basic::sample::SampleRun;
use tungsten_template_basic::sample::components::Player;
use tungsten_template_basic::shell::{self as game_shell, Difficulty, GameInfo, RunStart};

/// The game's config with its user folder set: `Off` unless a test checks
/// what persists, so tests never touch the developer's own files.
pub fn config(user_folder: UserFolder) -> Config {
    let mut config = Config::load("tungsten.json").expect("tungsten.json loads");
    config.game.user_folder = user_folder;
    config
}

/// What `main.rs` hands the shell: the window's title from `tungsten.json`.
pub fn info() -> GameInfo {
    GameInfo {
        title: config(UserFolder::Off).window.title,
        version: env!("CARGO_PKG_VERSION").into(),
    }
}

/// The app as `main.rs` builds it, with no window.
pub fn app(user_folder: UserFolder) -> App {
    let mut app = App::new(config(user_folder)).expect("App::new failed");
    app.set_exit_on_escape(false);
    game::register(&mut app, info());
    app
}

/// The game with its manifest's headless sections loaded and one frame
/// stepped: the title on top.
pub fn harness() -> UiHarness {
    harness_with(UserFolder::Off)
}

pub fn harness_with(user_folder: UserFolder) -> UiHarness {
    let mut harness = UiHarness::new(app(user_folder));
    harness
        .harness_mut()
        .load_manifest(game::MANIFEST)
        .expect("the manifest loads");
    harness.step(1);
    harness
}

/// A fresh user folder for one persistence test.
pub fn user_folder(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tungsten-template-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// A slot's file in `dir`'s saves.
pub fn slot_file(dir: &Path, slot: &str) -> PathBuf {
    dir.join("saves").join(format!("{slot}.json"))
}

/// The state on top.
pub fn top(harness: &UiHarness) -> Option<StateId> {
    game_shell::top(harness.world())
}

/// Steps until no screen change runs: a push the dispatcher has not
/// applied, a closing screen, the curtain or an engine transition.
pub fn settle(harness: &mut UiHarness) {
    let mut frames = 0;
    while game_shell::transitioning(harness.world()) {
        if harness.step(1) == 0 {
            return;
        }
        frames += 1;
        assert!(frames < 200, "the transition never ended");
    }
}

/// Presses `action` for a frame, releases it the next, then waits out the
/// transition it started.
pub fn tap(harness: &mut UiHarness, action: &str) {
    harness.harness_mut().press_action(action);
    harness.step(1);
    harness.harness_mut().release_action(action);
    harness.step(1);
    settle(harness);
}

/// The focusable control named `name` on the front-most shown screen.
pub fn find(harness: &UiHarness, name: &str) -> Option<WidgetId> {
    let tree = harness.tree();
    tree.roots().iter().rev().find_map(|root| {
        if tree.visibility(*root) != Some(Visibility::Visible) {
            return None;
        }
        let mut stack = vec![*root];
        while let Some(id) = stack.pop() {
            if tree.accessible_name(id) == Some(name) && tree.can_focus(id) {
                return Some(id);
            }
            stack.extend(tree.children(id).iter().rev().copied());
        }
        None
    })
}

/// The front-most shown screen's root.
pub fn shown_modal(harness: &UiHarness) -> Option<WidgetId> {
    let tree = harness.tree();
    let shown = |root: &&WidgetId| {
        tree.visibility(**root) == Some(Visibility::Visible) && tree.modal(**root)
    };
    tree.roots().iter().rev().find(shown).copied()
}

/// The widget named `name` on the front-most shown screen, enabled or not.
pub fn named(harness: &UiHarness, name: &str) -> Option<WidgetId> {
    let tree = harness.tree();
    let root = shown_modal(harness)?;
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        if tree.accessible_name(id) == Some(name) {
            return Some(id);
        }
        stack.extend(tree.children(id).iter().copied());
    }
    None
}

/// Whether a label on the front-most shown screen reads `text`.
pub fn shows(harness: &UiHarness, text: &str) -> bool {
    let tree = harness.tree();
    let Some(root) = shown_modal(harness) else {
        return false;
    };
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        if game_shell::widgets::text_of(tree, id) == text {
            return true;
        }
        stack.extend(tree.children(id).iter().copied());
    }
    false
}

/// Clicks the control named `name` on the front-most shown screen and steps
/// the frame that routes the click.
pub fn click(harness: &mut UiHarness, name: &str) {
    let id = find(harness, name).unwrap_or_else(|| panic!("no enabled control '{name}'"));
    harness.click(id);
    harness.step(1);
}

/// Focuses the control named `name` and activates it with `ui_accept`.
pub fn press(harness: &mut UiHarness, name: &str) {
    let id = find(harness, name).unwrap_or_else(|| panic!("no enabled control '{name}'"));
    assert!(harness.focus(id), "'{name}' takes focus");
    tap(harness, "ui_accept");
}

/// Starts a run at `difficulty` as the title's New Game does, and steps
/// into it through the curtain, returning once the curtain has lifted: the
/// game clock has run through its fade out, about 0.14 s into the run.
pub fn start_at(harness: &mut UiHarness, difficulty: Difficulty) {
    game_shell::start_run(
        harness.world_mut(),
        RunStart {
            difficulty,
            save: None,
        },
        None,
    );
    settle(harness);
    assert_eq!(top(harness), Some(game::GAMEPLAY));
}

pub fn start(harness: &mut UiHarness) {
    start_at(harness, Difficulty::Normal);
}

/// The player's drawn position.
pub fn player(harness: &UiHarness) -> Option<[f32; 2]> {
    harness
        .world()
        .query_filtered::<&Transform, With<Player>>()
        .next()
        .map(|transform| [transform.position.x, transform.position.y])
}

/// The player's body position: where the mover left it, which the drawn
/// `Transform` trails by up to a step.
pub fn player_position(harness: &UiHarness) -> Option<[f32; 2]> {
    let player = harness.world().get_resource::<SampleRun>()?.player;
    let at = harness.world().get::<Position>(player)?.0;
    Some([at.x, at.y])
}

/// Releases `action` and steps until the player's body stands still: the
/// mover slows to a stop over a few steps.
pub fn release_and_settle(harness: &mut UiHarness, action: &str) {
    harness.harness_mut().release_action(action);
    let player = player_entity(harness);
    for _ in 0..30 {
        harness.step(1);
        let still = harness.world().get::<Velocity>(player);
        if still.is_none_or(|velocity| velocity.0.length_squared() == 0.0) {
            return;
        }
    }
    panic!("the player never stopped");
}

/// The run's player.
pub fn player_entity(harness: &UiHarness) -> Entity {
    harness
        .world()
        .get_resource::<SampleRun>()
        .expect("a run is live")
        .player
}

pub fn health(harness: &UiHarness, entity: Entity) -> (f32, f32) {
    let health = harness.world().get::<Health>(entity).expect("a health");
    (health.current(), health.max())
}

/// The audio commands of `frames` frames, stepped one at a time.
pub fn heard(harness: &mut UiHarness, frames: u32) -> Vec<AudioCommand> {
    let mut commands = Vec::new();
    for _ in 0..frames {
        harness.step(1);
        commands.extend(harness.harness().audio().iter().cloned());
    }
    commands
}

/// Taps `action` and steps until no screen change runs, with every frame's
/// audio.
pub fn act(harness: &mut UiHarness, action: &str) -> Vec<AudioCommand> {
    harness.harness_mut().press_action(action);
    let mut commands = heard(harness, 1);
    harness.harness_mut().release_action(action);
    commands.extend(heard(harness, 1));
    while tungsten_template_basic::shell::transitioning(harness.world()) {
        commands.extend(heard(harness, 1));
    }
    commands.extend(heard(harness, 2));
    commands
}

/// The manifest's sound `name`.
pub fn sound(harness: &UiHarness, name: &str) -> AudioHandle {
    let sounds = harness.world().resource::<SoundRegistry>();
    sounds.get_by_id(name).expect("a manifest sound")
}

/// The voices started for `name` among `commands`, with their loop flag
/// and fade.
pub fn plays(
    harness: &UiHarness,
    commands: &[AudioCommand],
    name: &str,
) -> Vec<(Voice, bool, f32)> {
    let handle = sound(harness, name);
    commands
        .iter()
        .filter_map(|command| match command {
            AudioCommand::Play {
                handle: played,
                voice,
                looping,
                fade,
                ..
            } if *played == handle => Some((*voice, *looping, *fade)),
            _ => None,
        })
        .collect()
}

/// The stops among `commands`, with their fades.
pub fn stops(commands: &[AudioCommand]) -> Vec<(Voice, f32)> {
    commands
        .iter()
        .filter_map(|command| match command {
            AudioCommand::StopVoice { voice, fade, .. } => Some((*voice, *fade)),
            _ => None,
        })
        .collect()
}
