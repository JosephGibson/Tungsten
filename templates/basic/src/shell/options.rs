//! The player's gameplay options, beside the engine's `Settings`: their own
//! `SaveData` type in their own store, the slot `options` of
//! `SaveSlots::new(<user folder>/options)`, so the save slots never list
//! them. A damaged file logs, toasts and falls back to the defaults.

use serde::{Deserialize, Serialize};
use tungsten::core::save::SaveError;
use tungsten::core::{CameraController, SaveData, SaveSlots, World};

use super::toast;

const SLOT: &str = "options";

/// The gameplay options: a `World` resource.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GameOptions {
    /// Silence the game while its window has no focus.
    pub mute_unfocused: bool,
    /// Pause gameplay when its window loses focus.
    pub pause_unfocused: bool,
    /// Float the damage a hit deals.
    pub damage_numbers: bool,
    /// Write the autosaves: at a run's start, at pause, on quitting and at
    /// the exit, and when the game asks.
    pub autosave: bool,
    /// Draw the game's Bloom passes; off, the shell takes them out of its
    /// `PostStack` until the option comes back on.
    pub bloom: bool,
    screen_shake: f32,
}

impl Default for GameOptions {
    fn default() -> Self {
        Self {
            mute_unfocused: false,
            pause_unfocused: true,
            damage_numbers: true,
            autosave: true,
            bloom: true,
            screen_shake: 1.0,
        }
    }
}

impl SaveData for GameOptions {
    const SCHEMA: u32 = 1;
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

/// The options' store.
struct OptionsStore {
    slots: SaveSlots,
}

/// Loads the options into the world: `ShellPlugin`'s build calls it, and
/// `shell_start` applies them.
pub(crate) fn load(world: &mut World) {
    let dir = world
        .get_resource::<SaveSlots>()
        .and_then(SaveSlots::dir)
        .and_then(|saves| saves.parent())
        .map(|user| user.join(SLOT));
    let slots = SaveSlots::new(dir);
    let options = match slots.load::<GameOptions>(SLOT) {
        Ok(mut options) => {
            // Serde writes the fields as they are; the setter checks them.
            let shake = options.screen_shake;
            options.screen_shake = 1.0;
            options.set_screen_shake(shake);
            options
        }
        Err(SaveError::NotFound { .. } | SaveError::NoFolder) => GameOptions::default(),
        Err(error) => {
            log::warn!("Options not loaded, so the defaults apply: {error}");
            toast::toast(world, "Options could not be read; using the defaults");
            GameOptions::default()
        }
    };
    world.insert_resource(OptionsStore { slots });
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

/// Writes the options; a failure logs.
pub fn save(world: &World) {
    let store = world.resource::<OptionsStore>();
    match store.slots.save(SLOT, world.resource::<GameOptions>()) {
        Ok(()) | Err(SaveError::NoFolder) => {}
        Err(error) => log::warn!("Options not saved: {error}"),
    }
}
