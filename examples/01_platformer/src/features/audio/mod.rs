//! The audio: the exploration loop a run plays, quieted under pause and
//! game over, whose music the shell's states name; the combat layer
//! over it while an enemy sees the player; the wisps' hum on one shared
//! voice, louder as the nearest wisp comes near, the black holes' on
//! another, by the nearest open hole, and the fires' crackle on a third, by
//! the nearest burning thing; each area's ambience loop, raised in its
//! area; and the game's one-shot sounds as cue events about the player,
//! which the `cue_rules/sounds.json` data plays through the kit. The kit's
//! `CueListener`, at the player, quiets the one-shots and the three placed
//! loops by their distance from it.

pub mod components;
pub mod systems;

use glam::Vec2;
use tungsten::Plugin;
use tungsten::core::{AudioCommands, EventQueue, Schedule, SoundRegistry, Stage, World, system};
use tungsten::kit::{CUES, CueEvent, CueListener, CuesOnEvent, Phase, ensure_phases};

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

/// The audio as a plugin: each [`Sound`] a cue event the kit's cues read,
/// before the manifest's `cue_rules` load and name them; in the fixed
/// step's react phase `audio_listen` before the cues; and in `Update`
/// `audio_duck`, then `audio_hum`, `audio_holes`, `audio_crackle`,
/// `audio_combat` and `audio_ambience`.
pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        for sound in Sound::ALL {
            CuesOnEvent::<Sounded>::new(sound.event(), cue_of(sound)).build(schedule, world);
        }
        ensure_phases(schedule);
        // Before the kit's cues, which place the step's sounds by it.
        let listen = system("audio_listen", systems::listen).before(CUES);
        schedule.add(Stage::FixedUpdate, Phase::React.join(listen));
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
        Sound::Placed => occurrence::<9>,
        Sound::Steamed => occurrence::<10>,
        Sound::FrostBlasted => occurrence::<11>,
        Sound::Thundered => occurrence::<12>,
        Sound::Crackled => occurrence::<13>,
        Sound::Shot => occurrence::<14>,
        Sound::Reloaded => occurrence::<15>,
        Sound::Ricocheted => occurrence::<16>,
        Sound::WispFlare => occurrence::<17>,
        Sound::CrawlerHiss => occurrence::<18>,
        Sound::CrawlerLunge => occurrence::<19>,
        Sound::BeetlePaw => occurrence::<20>,
        Sound::BeetleCharge => occurrence::<21>,
        Sound::BatScreech => occurrence::<22>,
        Sound::BatDive => occurrence::<23>,
        Sound::TrollStep => occurrence::<24>,
        Sound::TrollRoar => occurrence::<25>,
        Sound::TrollSlam => occurrence::<26>,
        Sound::TrollThrow => occurrence::<27>,
        Sound::Splintered => occurrence::<28>,
        Sound::StaffGuttered => occurrence::<29>,
        Sound::LavaSet => occurrence::<30>,
        Sound::LiftStarted => occurrence::<31>,
        Sound::LiftStopped => occurrence::<32>,
        Sound::HoleCollapsed => occurrence::<33>,
        Sound::IronLanded => occurrence::<34>,
        Sound::Fizzled => occurrence::<35>,
        Sound::Snapped => occurrence::<36>,
        Sound::BallThudded => occurrence::<37>,
        Sound::DryFired => occurrence::<38>,
        Sound::Racked => occurrence::<39>,
        Sound::FireEnded => occurrence::<40>,
        Sound::AirJumped => occurrence::<41>,
        Sound::NoMana => occurrence::<42>,
        Sound::Switched => occurrence::<43>,
        Sound::OrbDropped => occurrence::<44>,
        Sound::CrateIgnited => occurrence::<45>,
        Sound::CrateCollapsed => occurrence::<46>,
        Sound::FuseLit => occurrence::<47>,
        Sound::KegBlasted => occurrence::<48>,
        Sound::RockThrown => occurrence::<49>,
        Sound::RockHit => occurrence::<50>,
        Sound::GoblinCackled => occurrence::<51>,
        Sound::TrollSwiped => occurrence::<52>,
        Sound::TrollGrunted => occurrence::<53>,
        Sound::Alerted => occurrence::<54>,
        Sound::GoblinStabbed => occurrence::<55>,
        Sound::LavaTook => occurrence::<56>,
        Sound::LavaChilled => occurrence::<57>,
        Sound::LavaCracked => occurrence::<58>,
        Sound::LavaMelted => occurrence::<59>,
        Sound::TomePactTaken => occurrence::<60>,
        Sound::TomeBoundingTaken => occurrence::<61>,
        Sound::TomeArcaneTaken => occurrence::<62>,
        Sound::Bounded => occurrence::<63>,
        Sound::BeetlePloughed => occurrence::<64>,
        Sound::CrawlerHopped => occurrence::<65>,
        Sound::SnowballThrown => occurrence::<66>,
        Sound::SnowballBurst => occurrence::<67>,
        Sound::GoblinCrouched => occurrence::<68>,
        Sound::GoblinDashed => occurrence::<69>,
        Sound::GoblinYelped => occurrence::<70>,
        Sound::GroundStruck => occurrence::<71>,
        Sound::CrateBroke => occurrence::<72>,
        Sound::GeyserSputter => occurrence::<73>,
        Sound::GeyserErupt => occurrence::<74>,
        Sound::StoneKnock => occurrence::<75>,
        Sound::FlaskBurst => occurrence::<76>,
        Sound::Corroded => occurrence::<77>,
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
/// the holes' hum, the fires' crackle, the combat layer and each area's
/// ambience loop start silent; and the listener, at the player.
pub fn start(world: &mut World) {
    let at = enemies::systems::player(world).map_or(Vec2::ZERO, |(_, at)| at);
    world.insert_resource(CueListener::new(LISTEN_FULL, LISTEN_SILENT).at(at));
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
