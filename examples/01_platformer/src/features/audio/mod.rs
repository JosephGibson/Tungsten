//! The audio: the exploration loop a run plays, quieted under pause's loop
//! and game over's music, as the template's sample does; the combat layer
//! over it while an enemy sees the player; the wisps' hum on one shared
//! voice, louder as the nearest wisp comes near; each area's ambience loop,
//! raised in its area; and the game's one-shot sounds as cue events about
//! the player, which the `cue_rules/sounds.json` data plays through the
//! kit.

pub mod components;
pub mod systems;

use glam::Vec2;
use tungsten::Plugin;
use tungsten::core::{AudioCommands, EventQueue, Schedule, SoundRegistry, Stage, World, system};
use tungsten::kit::{CueEvent, CuesOnEvent};

use crate::features::level::components::Area;
use crate::features::run::Run;
use crate::shell;
use components::{Ambience, Music, Sound, Sounded};

/// The run's loop and pause's, by manifest ID, and the seconds they fade
/// over: a run's start and end, and pause coming on top or going.
pub const MUSIC_EXPLORE: &str = "music_explore";
pub const MUSIC_PAUSE: &str = "music_pause";
pub const MUSIC_FADE: f32 = 0.5;
pub const PAUSE_FADE: f32 = 0.3;
/// The combat layer, by manifest ID, and the seconds it fades in over while
/// an awake enemy sees the player and out over after the last stops.
pub const MUSIC_COMBAT: &str = "music_combat";
pub const COMBAT_IN: f32 = 1.5;
pub const COMBAT_OUT: f32 = 3.0;
/// The wisps' hum, its reach from the nearest wisp, and the seconds its
/// volume moves over.
pub const WISP_HUM: &str = "ex10_wisp_hum_sfx";
pub const HUM_REACH: f32 = 600.0;
pub const HUM_FADE: f32 = 0.15;
/// The seconds an area's ambience rises over as the player enters it, and
/// the others fall over.
pub const AMBIENCE_FADE: f32 = 1.5;

/// The audio as a plugin: each [`Sound`] a cue event the kit's cues read,
/// before the manifest's `cue_rules` load and name them, and in `Update`
/// `audio_duck`, then `audio_hum`, `audio_combat` and `audio_ambience`.
pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        for sound in Sound::ALL {
            CuesOnEvent::<Sounded>::new(sound.event(), cue_of(sound)).build(schedule, world);
        }
        schedule.add(Stage::Update, system("audio_duck", systems::duck));
        let hum = system("audio_hum", systems::hum).after("audio_duck");
        schedule.add(Stage::Update, hum);
        let combat = system("audio_combat", systems::combat).after("audio_duck");
        schedule.add(Stage::Update, combat);
        let ambience = system("audio_ambience", systems::ambience).after("audio_duck");
        schedule.add(Stage::Update, ambience);
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
        Sound::Steamed => occurrence::<13>,
        Sound::FrostBlasted => occurrence::<14>,
        Sound::Thundered => occurrence::<15>,
        Sound::Crackled => occurrence::<16>,
        Sound::Shot => occurrence::<17>,
        Sound::Reloaded => occurrence::<18>,
        Sound::Ricocheted => occurrence::<19>,
        Sound::Summoned => occurrence::<20>,
        Sound::WispFlare => occurrence::<21>,
        Sound::CrawlerHiss => occurrence::<22>,
        Sound::CrawlerLunge => occurrence::<23>,
        Sound::BeetlePaw => occurrence::<24>,
        Sound::BeetleCharge => occurrence::<25>,
        Sound::BatScreech => occurrence::<26>,
        Sound::BatDive => occurrence::<27>,
        Sound::TrollStep => occurrence::<28>,
        Sound::TrollRoar => occurrence::<29>,
        Sound::TrollSlam => occurrence::<30>,
        Sound::TrollThrow => occurrence::<31>,
        Sound::Splintered => occurrence::<32>,
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

/// Starts a run's music: the exploration loop fades in, and the wisps' hum,
/// the combat layer and each area's ambience loop start silent.
pub fn start(world: &mut World) {
    let explore = shell::play(world, MUSIC_EXPLORE, MUSIC_FADE);
    let silent = |world: &mut World, id: &str| {
        let handle = world.resource::<SoundRegistry>().get_by_id(id)?;
        Some(
            world
                .resource_mut::<AudioCommands>()
                .play_with(handle, 0.0, true),
        )
    };
    let hum = silent(world, WISP_HUM);
    let combat = silent(world, MUSIC_COMBAT);
    world.insert_resource(Music {
        explore,
        hum,
        combat,
        ..Music::default()
    });
    let mut ids: Vec<String> = world
        .query::<&Area>()
        .filter_map(|area| area.ambience.clone())
        .collect();
    ids.sort();
    ids.dedup();
    let loops = ids
        .into_iter()
        .filter_map(|id| silent(world, &id).map(|voice| (id, voice, 0.0)))
        .collect();
    world.insert_resource(Ambience { loops });
}

/// Stops a run's music, pause's loop, the hum, the combat layer and the
/// ambience too: a Quit to Title from pause leaves the stack in one pass,
/// with gameplay never on top.
pub fn stop(world: &mut World) {
    let music = world.remove_resource::<Music>().unwrap_or_default();
    let ambience = world.remove_resource::<Ambience>().unwrap_or_default();
    let audio = world.resource_mut::<AudioCommands>();
    for voice in [music.explore, music.pause, music.hum, music.combat]
        .into_iter()
        .flatten()
        .chain(ambience.loops.iter().map(|(_, voice, _)| *voice))
    {
        audio.stop_voice(voice, MUSIC_FADE);
    }
}
