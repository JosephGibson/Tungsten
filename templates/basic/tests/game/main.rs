//! The game on the engine's headless harness, laid out after each frame: no
//! window or renderer, the frames stepped one call at a time. `sample` holds
//! the sample's gameplay, `shell` the shell's screens and run, `saves` the
//! save seam and its screen, `settings` the tabbed settings, `keys` the
//! keybindings, `fit` every screen at six windows and the scale extremes,
//! `screens` the shell's screen files and `visual` the title's pixels.

mod fit;
mod keys;
mod sample;
mod saves;
mod screens;
mod script;
mod settings;
mod shell;
mod visual;

use std::path::{Path, PathBuf};

use tungsten::core::{Config, Entity, Position, Transform, UserFolder, Velocity, With};
use tungsten::kit::{Health, Source};
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

/// Taps `action`, then waits out the transition it started.
pub fn tap(harness: &mut UiHarness, action: &str) {
    harness.tap_action(action);
    settle(harness);
}

/// Clicks the control named `name` on the front-most shown screen and steps
/// the frame that routes the click.
pub fn click(harness: &mut UiHarness, name: &str) {
    let id = harness
        .find(name)
        .unwrap_or_else(|| panic!("no enabled control '{name}'"));
    harness.click(id);
    harness.step(1);
}

/// Focuses the control named `name` and activates it with `ui_accept`.
pub fn press(harness: &mut UiHarness, name: &str) {
    let id = harness
        .find(name)
        .unwrap_or_else(|| panic!("no enabled control '{name}'"));
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

/// The standing enemy the spawner at `slot` of the sample's `ENEMIES`
/// keeps: the one whose kit `Source` is that spawner.
pub fn enemy(harness: &UiHarness, slot: usize) -> Option<Entity> {
    let world = harness.world();
    let spawner = world.get_resource::<SampleRun>()?.spawners[slot].1;
    world
        .query::<(Entity, &Source, &Health)>()
        .find(|(_, source, health)| source.entity() == spawner && !health.is_dead())
        .map(|(entity, ..)| entity)
}
