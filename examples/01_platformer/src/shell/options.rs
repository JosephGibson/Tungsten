//! The player's gameplay options: the game section of the engine's
//! `Settings` (`settings.json`'s `game`), read once into a `World` resource,
//! the live copy, and written back by [`save`]. Bloom and the mute while the
//! window has no focus are the engine's own switches (`Settings::bloom`,
//! `Settings::mute_unfocused`). The options file of a shell before the game
//! section, `<user folder>/options/options.json`, is read once, into both,
//! while `Settings` holds no game section.

use serde::de::IgnoredAny;
use serde::{Deserialize, Serialize};
use tungsten::core::settings::SettingsError;
use tungsten::core::{CameraController, SaveData, SaveSlots, Settings, World};

/// The older shell's options slot and its folder under the user folder.
const OLD: &str = "options";

/// The gameplay options: a `World` resource.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GameOptions {
    /// Pause gameplay when its window loses focus.
    pub pause_unfocused: bool,
    /// Float the damage a hit deals.
    pub damage_numbers: bool,
    /// Write the autosaves: at a run's start, at pause, on quitting and at
    /// the exit, and when the game asks.
    pub autosave: bool,
    screen_shake: f32,
}

impl Default for GameOptions {
    fn default() -> Self {
        Self {
            pause_unfocused: true,
            damage_numbers: true,
            autosave: true,
            screen_shake: 1.0,
        }
    }
}

impl GameOptions {
    /// How much of the shipped screen shake plays, 0 to 1.
    #[must_use]
    pub fn screen_shake(&self) -> f32 {
        self.screen_shake
    }

    /// Sets the screen shake, clamped to 0..=1; a value that is not finite
    /// is ignored.
    pub fn set_screen_shake(&mut self, shake: f32) {
        if shake.is_finite() {
            self.screen_shake = shake.clamp(0.0, 1.0);
        }
    }
}

/// The older shell's options file: the options with the two switches the
/// engine now keeps.
#[derive(Serialize, Deserialize)]
#[serde(default)]
struct OldOptions {
    mute_unfocused: bool,
    bloom: bool,
    #[serde(flatten)]
    options: GameOptions,
}

impl Default for OldOptions {
    fn default() -> Self {
        Self {
            mute_unfocused: false,
            bloom: true,
            options: GameOptions::default(),
        }
    }
}

impl SaveData for OldOptions {
    const SCHEMA: u32 = 1;
}

/// Reads the older shell's options file into `Settings` while they hold no
/// game section: Bloom off and the mute on go to the engine's switches, the
/// rest to the section, and `Settings` save, so the file is read once.
fn adopt_old(world: &mut World) {
    let settings = world.resource::<Settings>();
    if settings.game::<Option<IgnoredAny>>().is_some() {
        return;
    }
    let dir = world
        .get_resource::<SaveSlots>()
        .and_then(SaveSlots::dir)
        .and_then(|saves| saves.parent())
        .map(|user| user.join(OLD));
    let Ok(old) = SaveSlots::new(dir).load::<OldOptions>(OLD) else {
        return;
    };
    let settings = world.resource_mut::<Settings>();
    if !old.bloom {
        settings.set_bloom(Some(false));
    }
    if old.mute_unfocused {
        settings.set_mute_unfocused(Some(true));
    }
    settings.set_game(&old.options);
    if let Err(error) = settings.save() {
        log::warn!("The old options were read but not saved: {error}");
    }
}

/// Loads the options into the world from `Settings`' game section, the
/// defaults without one: `ShellPlugin`'s build calls it, and `shell_start`
/// applies them.
pub(crate) fn load(world: &mut World) {
    adopt_old(world);
    let mut options: GameOptions = world.resource::<Settings>().game();
    // Serde writes the fields as they are; the setter checks them.
    let shake = options.screen_shake;
    options.screen_shake = 1.0;
    options.set_screen_shake(shake);
    world.insert_resource(options);
}

/// Applies the options the engine reads: the camera's shake scale, the
/// share of the game's own shake that plays.
pub fn apply(world: &mut World) {
    let shake = world.resource::<GameOptions>().screen_shake;
    if let Some(camera) = world.get_resource_mut::<CameraController>() {
        camera.shake_scale = shake;
    }
}

/// Writes `Settings`, the options in their game section; a failure logs,
/// and no user folder saves nothing.
pub fn save(world: &mut World) {
    let options = *world.resource::<GameOptions>();
    let settings = world.resource_mut::<Settings>();
    settings.set_game(&options);
    match settings.save() {
        Ok(()) | Err(SettingsError::NoFolder) => {}
        Err(error) => log::warn!("Settings not saved: {error}"),
    }
}
