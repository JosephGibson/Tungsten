//! The seam between the shell and the game: the names the shell calls,
//! which a new game edits to point at its own gameplay, and `register`, which
//! adds the kit, the shell and the game. `save.rs` holds the rest of it.

use tungsten::kit::KitPlugins;
use tungsten::{App, GameState, StateId};

use crate::sample::{GameplayState, SamplePlugin, max_health};
use crate::shell::{Difficulty, GameInfo, RunStart, ShellPlugin, boot};

/// The game's manifest, relative to its folder.
pub const MANIFEST: &str = "assets/manifest.json";
/// The manifest's font for the game's text.
pub const FONT: &str = "sans";

/// The gameplay state's ID: pause opens over it, and losing focus pauses
/// it.
pub const GAMEPLAY: StateId = "gameplay";

/// The state the shell puts on the stack for New Game, Continue, Load and
/// Retry.
#[must_use]
pub fn gameplay(start: RunStart) -> impl GameState {
    GameplayState::new(start)
}

/// What `difficulty` changes, on its card in the New Game picker: short
/// facts joined by " · ", each a line of its own under the difficulty's
/// name and rank, as short as the sample's `8 health`, since three cards
/// share a row, which `tests/game/fit.rs` checks at text 200 %.
#[must_use]
pub fn difficulty_note(difficulty: Difficulty) -> String {
    format!("{} health", max_health(difficulty))
}

/// Registers the game with `app`: the kit's plugins, the shell, then the
/// sample, and the shell's boot splash. The kit runs everything the
/// sample's prefabs compose: the player's eased movement, aim and shot,
/// health, hit flash, knockback and camera, the bolt's flight and hit, the
/// hazard's patrol, touch and slow, the dummy's mend, the gems' drift and
/// count, and the pickups' heal and speed.
pub fn register(app: &mut App, info: GameInfo) {
    app.add_plugins(KitPlugins::set());
    app.add_plugin(ShellPlugin::new(info));
    app.add_plugin(SamplePlugin);
    app.set_boot_view(boot::view);
}
