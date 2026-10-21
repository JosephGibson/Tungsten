//! The audio's systems.

use tungsten::core::{AudioCommands, SoundRegistry, World};

use super::components::Music;
use super::{MUSIC_EXPLORE, MUSIC_PAUSE, PAUSE_FADE};
use crate::game::GAMEPLAY;
use crate::shell::game_over::GAME_OVER;
use crate::shell::pause::PAUSE;
use crate::shell::{self};

/// `audio_duck`: when pause comes on top, the exploration loop falls to 0
/// and pause's loop fades in; when gameplay is back on top, pause's loop
/// stops and the exploration comes back. A screen over pause keeps the
/// loop. Game over silences the exploration too, under the music the shell
/// plays.
pub fn duck(world: &mut World) {
    let Some(music) = world.get_resource::<Music>().copied() else {
        return;
    };
    let top = shell::top(world);
    let paused = top == Some(PAUSE);
    let (on, off) = (paused || top == Some(GAME_OVER), top == Some(GAMEPLAY));
    if on == music.ducked || !(on || off) {
        return;
    }
    let pause = if paused {
        shell::play(world, MUSIC_PAUSE, PAUSE_FADE)
    } else {
        None
    };
    let registry = world.resource::<SoundRegistry>();
    let level = registry
        .get_by_id(MUSIC_EXPLORE)
        .map_or(1.0, |explore| registry.get_volume(explore));
    let audio = world.resource_mut::<AudioCommands>();
    if let Some(explore) = music.explore {
        audio.set_voice_volume(explore, if on { 0.0 } else { level }, PAUSE_FADE);
    }
    if let Some(looped) = music.pause {
        audio.stop_voice(looped, PAUSE_FADE);
    }
    world.insert_resource(Music {
        pause,
        ducked: on,
        ..music
    });
}
