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
use crate::features::player::{PlayerPlugin, max_health};
use crate::features::run::{GameplayState, RunPlugin};
use crate::features::spells::SpellsPlugin;
use crate::features::toys::ToysPlugin;
use crate::shell::{Difficulty, GameInfo, RunStart, ShellPlugin};

/// The game's manifest, relative to its folder.
pub const MANIFEST: &str = "assets/manifest.json";

/// The gameplay state's ID: pause opens over it, and losing focus pauses
/// it.
pub const GAMEPLAY: StateId = "gameplay";

/// The actions the Keybindings screen lists: each action's name and its
/// row's label. `ui_*` and `engine_*` actions are never listed, so the
/// menus cannot be lost.
pub const BINDABLE_ACTIONS: &[(&str, &str)] = &[
    ("move_left", "Move left"),
    ("move_right", "Move right"),
    ("jump", "Jump"),
    ("cast", "Cast"),
    ("shoot", "Shoot"),
    ("spell_next", "Next spell"),
    ("spell_prev", "Previous spell"),
    ("summon_next", "Next summon"),
    ("summon_prev", "Previous summon"),
    ("spell_1", "Spell 1"),
    ("spell_2", "Spell 2"),
    ("spell_3", "Spell 3"),
    ("spell_4", "Spell 4"),
    ("spell_5", "Spell 5"),
    ("spell_6", "Spell 6"),
    ("spell_7", "Spell 7"),
    ("spell_8", "Spell 8"),
    ("spell_9", "Spell 9"),
    ("spell_0", "Spell 0"),
    ("lantern", "Staff light"),
    ("zoom_in", "Zoom in"),
    ("zoom_out", "Zoom out"),
    ("state_pause", "Pause"),
];

/// The state the shell puts on the stack for New Game, Continue, Load and
/// Retry.
#[must_use]
pub fn gameplay(start: RunStart) -> impl GameState {
    GameplayState::new(start)
}

/// What `difficulty` changes, the note beside it in the New Game picker:
/// the player's health and the enemies' factor on their health and damage
/// (Q11).
#[must_use]
pub fn difficulty_note(difficulty: Difficulty) -> String {
    format!(
        "{} health · enemies ×{}",
        max_health(difficulty),
        enemies::factor(difficulty)
    )
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
    app.add_plugin(DummyPlugin);
    app.add_plugin(EnemiesPlugin);
    app.add_plugin(CombatPlugin);
    app.add_plugin(LightningPlugin);
    app.add_plugin(GunPlugin);
    app.add_plugin(SpellsPlugin);
    // Last, so the cursor draws over every other contribution.
    app.add_plugin(CursorPlugin);
}
