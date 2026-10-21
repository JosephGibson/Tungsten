//! The audio: the exploration loop a run plays, quieted under pause's loop
//! and game over's music, as the template's sample does, and the game's
//! one-shot sounds as cue events about the player, which the
//! `cue_rules/sounds.json` data plays through the kit.

pub mod components;
pub mod systems;

use glam::Vec2;
use tungsten::Plugin;
use tungsten::core::{AudioCommands, EventQueue, Schedule, Stage, World, system};
use tungsten::kit::{CueEvent, CuesOnEvent};

use crate::features::run::Run;
use crate::shell;
use components::{Music, Sound, Sounded};

/// The run's loop and pause's, by manifest ID, and the seconds they fade
/// over: a run's start and end, and pause coming on top or going.
pub const MUSIC_EXPLORE: &str = "music_explore";
pub const MUSIC_PAUSE: &str = "music_pause";
pub const MUSIC_FADE: f32 = 0.5;
pub const PAUSE_FADE: f32 = 0.3;

/// The audio as a plugin: each [`Sound`] a cue event the kit's cues read,
/// before the manifest's `cue_rules` load and name them, and
/// `audio_duck` in `Update`.
pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        for sound in Sound::ALL {
            CuesOnEvent::<Sounded>::new(sound.event(), cue_of(sound)).build(schedule, world);
        }
        schedule.add(Stage::Update, system("audio_duck", systems::duck));
    }
}

/// The occurrence of `S`, the sound's place in [`Sound::ALL`].
fn occurrence<const S: usize>(sounded: &Sounded) -> Option<CueEvent> {
    (sounded.sound == Sound::ALL[S]).then(|| CueEvent::new(sounded.subject, sounded.at))
}

/// The map of `sound`'s cue event.
fn cue_of(sound: Sound) -> fn(&Sounded) -> Option<CueEvent> {
    match sound {
        Sound::Blasted => occurrence::<0>,
        Sound::Crushed => occurrence::<1>,
        Sound::Doused => occurrence::<2>,
        Sound::Froze => occurrence::<3>,
        Sound::Shattered => occurrence::<4>,
        Sound::HoleOpened => occurrence::<5>,
        Sound::ColdStarted => occurrence::<6>,
        Sound::ColdEnded => occurrence::<7>,
        Sound::FireStarted => occurrence::<8>,
        Sound::Tossed => occurrence::<9>,
        Sound::Placed => occurrence::<10>,
        Sound::Cast => occurrence::<11>,
        Sound::Pushed => occurrence::<12>,
    }
}

/// Plays `sound`, heard at `at`, through its cue: at the next cue step,
/// while a run's player lives to hear it.
pub fn play(world: &mut World, sound: Sound, at: Vec2) {
    let Some(subject) = world.get_resource::<Run>().map(|run| run.player) else {
        return;
    };
    if let Some(queue) = world.get_resource_mut::<EventQueue<Sounded>>() {
        queue.send(Sounded { sound, at, subject });
    }
}

/// Starts a run's music: the exploration loop fades in.
pub fn start(world: &mut World) {
    let explore = shell::play(world, MUSIC_EXPLORE, MUSIC_FADE);
    world.insert_resource(Music {
        explore,
        ..Music::default()
    });
}

/// Stops a run's music, pause's loop too: a Quit to Title from pause
/// leaves the stack in one pass, with gameplay never on top.
pub fn stop(world: &mut World) {
    let music = world.remove_resource::<Music>().unwrap_or_default();
    let audio = world.resource_mut::<AudioCommands>();
    for voice in [music.explore, music.pause].into_iter().flatten() {
        audio.stop_voice(voice, MUSIC_FADE);
    }
}
