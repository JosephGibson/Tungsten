//! The seam between the shell and the game: the names the shell calls and
//! `register`, which adds the kit, the shell and the game's features.
//! `save.rs` holds the rest of it.

use tungsten::kit::{ChemistryPlugin, KitPlugins};
use tungsten::{App, GameState, StateId};

use crate::features::audio::AudioPlugin;
use crate::features::combat::CombatPlugin;
use crate::features::dummy::DummyPlugin;
use crate::features::elements::{CASCADE_BOUND, CASCADE_CAP, ElementsPlugin, cascade, systems};
use crate::features::enemies::{self, EnemiesPlugin};
use crate::features::gun::GunPlugin;
use crate::features::level::{CursorPlugin, LevelPlugin, lava};
use crate::features::lightning::LightningPlugin;
use crate::features::player::PlayerPlugin;
use crate::features::props::{self, PropsPlugin};
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

/// 01's chemistry: its cascade's cap a run and bound (Q4), and no depth
/// limit to speak of, since its cascade's chains had none and its rules
/// make no cycle.
pub const CHEMISTRY_DEPTH: u8 = u8::MAX;

/// The kit's plugins with 01's chemistry in place of the set's own, at its
/// cascade's settings ([`CHEMISTRY_DEPTH`]), with its handlers, 01's own
/// outputs where the kit's differ (the plan's §2.1): `freeze`, a coat's
/// band freezing its body for its seconds; `thermal_shock`, `shatter` and
/// `steam`, the endings' blasts, douses and ends; `light`, a ball, a crate,
/// a plank or a keg's fuse catching; `detonate`, a keg or a bomb blowing,
/// and `fuse_out`, its fuse's end setting it off unless put out;
/// `burn_out`, a crate or a plank burnt out falling in; `harden` and
/// `melt`, a basin's crust setting and melting; `relight`, a fire hazard's
/// douse ending.
#[must_use]
pub fn kit_plugins() -> tungsten::core::PluginSet {
    KitPlugins::set().with(
        ChemistryPlugin::new()
            .with_cap(CASCADE_CAP)
            .with_bound(CASCADE_BOUND)
            .with_depth(CHEMISTRY_DEPTH)
            .with_handler("freeze", systems::freeze_handler)
            .with_handler("thermal_shock", cascade::thermal_shock_handler)
            .with_handler("shatter", cascade::shatter_handler)
            .with_handler("steam", cascade::steam_handler)
            .with_handler("light", cascade::light_handler)
            .with_handler("detonate", cascade::detonate_handler)
            .with_handler("fuse_out", cascade::fuse_out_handler)
            .with_handler("burn_out", props::burn_out_handler)
            .with_handler("harden", lava::harden_handler)
            .with_handler("melt", lava::melt_handler)
            .with_handler("relight", systems::relight_handler),
    )
}

/// Registers the game with `app`: the kit's plugins ([`kit_plugins`]), the
/// shell, then the features, the cursor last.
pub fn register(app: &mut App, info: GameInfo) {
    app.add_plugins(kit_plugins());
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
