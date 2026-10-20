//! The seam between the shell and the game: the names the shell calls,
//! which a new game edits to point at its own gameplay, and `register`, which
//! adds the kit, the shell and the game. `save.rs` holds the rest of it.

use tungsten::kit::KitPlugins;
use tungsten::{App, GameState, StateId};

use crate::sample::{GameplayState, SamplePlugin, max_health};
use crate::shell::{Difficulty, GameInfo, RunStart, ShellPlugin};

/// The game's manifest, relative to its folder.
pub const MANIFEST: &str = "assets/manifest.json";
/// The manifest's font for the game's text.
pub const FONT: &str = "sans";

/// The gameplay state's ID: pause opens over it, and losing focus pauses
/// it.
pub const GAMEPLAY: StateId = "gameplay";

/// The actions the Keybindings screen lists: each action's name and its
/// row's label. `ui_*` and `engine_*` actions are never listed, so the
/// menus cannot be lost.
pub const BINDABLE_ACTIONS: &[(&str, &str)] = &[
    ("move_up", "Move up"),
    ("move_down", "Move down"),
    ("move_left", "Move left"),
    ("move_right", "Move right"),
    ("state_pause", "Pause"),
    ("shoot", "Shoot"),
];

/// The state the shell puts on the stack for New Game, Continue, Load and
/// Retry.
#[must_use]
pub fn gameplay(start: RunStart) -> impl GameState {
    GameplayState::new(start)
}

/// What `difficulty` changes, the muted note beside it in the New Game
/// picker: a few words, as short as the sample's `8 health`, since it
/// shares a menu-wide row with its button on one line, which
/// `tests/game/fit.rs` checks at text 200 %.
#[must_use]
pub fn difficulty_note(difficulty: Difficulty) -> String {
    format!("{} health", max_health(difficulty))
}

/// Registers the game with `app`: the kit's plugins, the shell, then the
/// sample. The kit runs everything the sample's prefabs compose: the
/// player's eased movement, aim and shot, health, hit flash, knockback and
/// camera, the bolt's flight and hit, the hazard's patrol, touch and slow,
/// the dummy's mend, the gems' drift and count, and the pickups' heal and
/// speed.
pub fn register(app: &mut App, info: GameInfo) {
    app.add_plugins(KitPlugins::set());
    app.add_plugin(ShellPlugin::new(info));
    app.add_plugin(SamplePlugin);
}
