//! The seam between the shell and the game: the names the shell calls and
//! `register`, which adds the kit, the shell and the game's features.
//! `save.rs` holds the rest of it.

use tungsten::kit::KitPlugins;
use tungsten::{App, GameState, StateId};

use crate::features::audio::AudioPlugin;
use crate::features::combat::CombatPlugin;
use crate::features::dummy::DummyPlugin;
use crate::features::elements::ElementsPlugin;
use crate::features::enemies::{self, EnemiesPlugin};
use crate::features::gun::GunPlugin;
use crate::features::level::{CursorPlugin, LevelPlugin};
use crate::features::lightning::LightningPlugin;
use crate::features::player::PlayerPlugin;
use crate::features::props::PropsPlugin;
use crate::features::run::{GameplayState, RunPlugin};
use crate::features::spells::SpellsPlugin;
use crate::features::tomes::TomesPlugin;
use crate::features::toys::ToysPlugin;
use crate::shell::{Difficulty, GameInfo, RunStart, ShellPlugin};

/// The game's manifest, relative to its folder.
pub const MANIFEST: &str = "assets/manifest.json";

/// The gameplay state's ID: pause opens over it, and losing focus pauses
/// it.
pub const GAMEPLAY: StateId = "gameplay";

/// The state the shell puts on the stack for New Game, Continue, Load and
/// Retry.
#[must_use]
pub fn gameplay(start: RunStart) -> impl GameState {
    GameplayState::new(start)
}

/// What `difficulty` changes, on its card in the New Game picker, a fact a
/// line: the enemies' factor on their health and on their damage (Q11);
/// the player's health is the same at each.
#[must_use]
pub fn difficulty_note(difficulty: Difficulty) -> String {
    let factor = enemies::factor(difficulty);
    format!("enemy health ×{factor} · enemy damage ×{factor}")
}

/// Registers the game with `app`: the kit's plugins, the shell, then the
/// features, the cursor last.
pub fn register(app: &mut App, info: GameInfo) {
    app.add_plugins(KitPlugins::set());
    app.add_plugin(ShellPlugin::new(info));
    app.add_plugin(LevelPlugin);
    app.add_plugin(PlayerPlugin);
    app.add_plugin(RunPlugin);
    app.add_plugin(AudioPlugin);
    app.add_plugin(ToysPlugin);
    app.add_plugin(ElementsPlugin);
    app.add_plugin(PropsPlugin);
    app.add_plugin(DummyPlugin);
    app.add_plugin(EnemiesPlugin);
    app.add_plugin(CombatPlugin);
    app.add_plugin(LightningPlugin);
    app.add_plugin(GunPlugin);
    app.add_plugin(SpellsPlugin);
    app.add_plugin(TomesPlugin);
    // Last, so the cursor draws over every other contribution.
    app.add_plugin(CursorPlugin);
}
