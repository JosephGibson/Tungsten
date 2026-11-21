//! The audio: the exploration loop a run plays, quieted under pause and
//! game over, whose music the shell's states name; the combat layer
//! over it while an enemy sees the player; the wisps' hum on one shared
//! voice, louder as the nearest wisp comes near, the black holes' on
//! another, by the nearest open hole, and the fires' crackle on a third, by
//! the nearest burning thing; each area's ambience loop, raised in its
//! area; and the game's one-shot sounds as cue events about the player,
//! which the `cue_rules/sounds.json` data plays through the kit. The kit's
//! `CueListener`, following the player's body, quiets the one-shots and the
//! three placed loops by their distance from it.

pub mod components;
pub mod systems;

use glam::Vec2;
use tungsten::Plugin;
use tungsten::core::{AudioCommands, EventQueue, Schedule, SoundRegistry, Stage, World, system};
use tungsten::kit::{CueEvent, CueListener, CuesOnEvent};

use crate::features::enemies;
use crate::features::level::components::Area;
use crate::features::run::Run;
use crate::shell;
use components::{Ambience, Music, Sound, Sounded};

/// The run's loop, by manifest ID, and the seconds it fades over: a run's
/// start and end, and pause or game over coming on top or going.
pub const MUSIC_EXPLORE: &str = "music_explore";
pub const MUSIC_FADE: f32 = 0.5;
pub const PAUSE_FADE: f32 = 0.3;
/// The combat layer, by manifest ID, and the seconds it fades in over while
/// an awake enemy sees the player and out over after the last stops.
pub const MUSIC_COMBAT: &str = "music_combat";
pub const COMBAT_IN: f32 = 1.5;
pub const COMBAT_OUT: f32 = 3.0;
/// The listener's hearing, world distances the Zoom option leaves alone: a
/// sound at its full volume within half a view's width at zoom 1, falling
/// to silence at one and a half views.
pub const LISTEN_FULL: f32 = 1024.0;
pub const LISTEN_SILENT: f32 = 3072.0;
/// The wisps' hum, and the seconds its volume moves over.
pub const WISP_HUM: &str = "ex10_wisp_hum_sfx";
pub const HUM_FADE: f32 = 0.15;
/// The black holes' hum; its volume moves over [`HUM_FADE`] as the wisps'
/// does.
pub const HOLE_HUM: &str = "ex10_black_hole_hum_sfx";
/// The fires' crackle; its volume moves over [`HUM_FADE`] as the wisps'
/// does.
pub const FIRE_CRACKLE: &str = "ex10_fire_crackle_sfx";
/// The seconds an area's ambience rises over as the player enters it, and
/// the others fall over.
pub const AMBIENCE_FADE: f32 = 1.5;

/// The audio as a plugin: every [`Sound`] a cue event the kit's cues read,
/// brought in by one registration under their names before the manifest's
/// `cue_rules` load and name them; and in `Update` `audio_duck`, then
/// `audio_hum`, `audio_holes`, `audio_crackle`, `audio_combat` and
/// `audio_ambience`. The kit's cues move the listener, from [`start`].
pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        CuesOnEvent::<Sounded>::with_names(&Sound::ALL.map(Sound::event), cue_of)
            .build(schedule, world);
        schedule.add(Stage::Update, system("audio_duck", systems::duck));
        let hum = system("audio_hum", systems::hum).after("audio_duck");
        schedule.add(Stage::Update, hum);
        let holes = system("audio_holes", systems::holes).after("audio_duck");
        schedule.add(Stage::Update, holes);
        let crackle = system("audio_crackle", systems::crackle).after("audio_duck");
        schedule.add(Stage::Update, crackle);
        let combat = system("audio_combat", systems::combat).after("audio_duck");
        schedule.add(Stage::Update, combat);
        let ambience = system("audio_ambience", systems::ambience).after("audio_duck");
        schedule.add(Stage::Update, ambience);
    }
}

/// A sound's occurrence, under its cue event's name.
fn cue_of(sounded: &Sounded) -> Option<CueEvent> {
    Some(CueEvent::new(sounded.subject, sounded.at).named(sounded.sound.event()))
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
/// the holes' hum, the fires' crackle, the combat layer and each area's
/// ambience loop start silent; and the listener, at the player, whose body
/// centre the kit's cues move it to each fixed step, through a death too.
pub fn start(world: &mut World) {
    let at = enemies::systems::player(world).map_or(Vec2::ZERO, |(_, at)| at);
    let listener = CueListener::new(LISTEN_FULL, LISTEN_SILENT).at(at);
    let listener = match world.get_resource::<Run>() {
        Some(run) => listener.following_body(run.player),
        None => listener,
    };
    world.insert_resource(listener);
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
    let holes = silent(world, HOLE_HUM);
    let crackle = silent(world, FIRE_CRACKLE);
    let combat = silent(world, MUSIC_COMBAT);
    world.insert_resource(Music {
        explore,
        hum,
        holes,
        crackle,
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

/// Stops a run's music, the hums, the crackle, the combat layer and the
/// ambience too, and removes the listener: a Quit to Title from pause
/// leaves the stack in one pass, with gameplay never on top.
pub fn stop(world: &mut World) {
    world.remove_resource::<CueListener>();
    let music = world.remove_resource::<Music>().unwrap_or_default();
    let ambience = world.remove_resource::<Ambience>().unwrap_or_default();
    let audio = world.resource_mut::<AudioCommands>();
    for voice in [
        music.explore,
        music.hum,
        music.holes,
        music.crackle,
        music.combat,
    ]
    .into_iter()
    .flatten()
    .chain(ambience.loops.iter().map(|(_, voice, _)| *voice))
    {
        audio.stop_voice(voice, MUSIC_FADE);
    }
}
