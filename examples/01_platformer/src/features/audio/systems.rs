//! The audio's systems.

use tungsten::core::{AudioCommands, SoundRegistry, World};
use tungsten::kit::{Health, StateMachine};

use super::components::{Ambience, Music};
use super::{
    AMBIENCE_FADE, COMBAT_IN, COMBAT_OUT, HUM_FADE, HUM_REACH, MUSIC_COMBAT, MUSIC_EXPLORE,
    MUSIC_PAUSE, PAUSE_FADE, WISP_HUM,
};
use crate::features::enemies;
use crate::features::enemies::components::{Enemy, EnemyKind, EnemyState, Sight};
use crate::features::level::systems::current_area;
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
    let mut hum_level = music.hum_level;
    if let (Some(hum), true) = (music.hum, on) {
        audio.set_voice_volume(hum, 0.0, PAUSE_FADE);
        hum_level = 0.0;
    }
    world.insert_resource(Music {
        pause,
        hum_level,
        ducked: on,
        ..music
    });
}

/// `audio_hum`, after the duck: while gameplay is on top, the wisps' hum
/// sounds at its level by the nearest live, thinking wisp, falling to 0 at
/// [`HUM_REACH`] from the player.
pub fn hum(world: &mut World) {
    let Some(music) = world.get_resource::<Music>().copied() else {
        return;
    };
    let Some(voice) = music.hum.filter(|_| !music.ducked) else {
        return;
    };
    let player = enemies::systems::player(world).map(|(_, at)| at);
    let nearest = player.and_then(|player| {
        world
            .query::<(
                tungsten::core::Entity,
                &Enemy,
                &Health,
                &StateMachine<EnemyState>,
            )>()
            .filter(|(_, enemy, health, machine)| {
                enemy.kind == EnemyKind::Wisp && !health.is_dead() && !machine.is_held()
            })
            .filter_map(|(entity, ..)| enemies::centre(world, entity))
            .map(|at| at.distance(player))
            .min_by(f32::total_cmp)
    });
    let registry = world.resource::<SoundRegistry>();
    let level = registry
        .get_by_id(WISP_HUM)
        .map_or(0.0, |handle| registry.get_volume(handle));
    let volume = nearest.map_or(0.0, |distance| {
        level * (1.0 - distance / HUM_REACH).clamp(0.0, 1.0)
    });
    if (volume - music.hum_level).abs() < 0.01 {
        return;
    }
    world
        .resource_mut::<AudioCommands>()
        .set_voice_volume(voice, volume, HUM_FADE);
    world.insert_resource(Music {
        hum_level: volume,
        ..music
    });
}

/// `audio_combat`, after the duck: the combat layer rises to its level over
/// [`COMBAT_IN`] while any live, thinking enemy sees the player and falls
/// to 0 over [`COMBAT_OUT`] after the last stops; ducked under pause and
/// game over, it falls to 0 with the exploration loop. Presentation.
pub fn combat(world: &mut World) {
    let Some(music) = world.get_resource::<Music>().copied() else {
        return;
    };
    let Some(voice) = music.combat else {
        return;
    };
    let seen = world
        .query::<(&Enemy, &Health, &StateMachine<EnemyState>, &Sight)>()
        .any(|(_, health, machine, sight)| !health.is_dead() && !machine.is_held() && sight.sees);
    let registry = world.resource::<SoundRegistry>();
    let level = registry
        .get_by_id(MUSIC_COMBAT)
        .map_or(0.0, |handle| registry.get_volume(handle));
    let volume = if seen && !music.ducked { level } else { 0.0 };
    if (volume - music.combat_level).abs() < 1e-4 {
        return;
    }
    let seconds = if music.ducked {
        PAUSE_FADE
    } else if volume > music.combat_level {
        COMBAT_IN
    } else {
        COMBAT_OUT
    };
    world
        .resource_mut::<AudioCommands>()
        .set_voice_volume(voice, volume, seconds);
    world.insert_resource(Music {
        combat_level: volume,
        ..music
    });
}

/// `audio_ambience`, after the duck: the current area's loop rises to its
/// manifest volume over [`AMBIENCE_FADE`] and every other falls to 0 over
/// the same; ducked under pause and game over, every loop falls to 0 with
/// the exploration loop. Presentation.
pub fn ambience(world: &mut World) {
    let (Some(mut ambience), Some(music)) = (
        world.get_resource::<Ambience>().cloned(),
        world.get_resource::<Music>().copied(),
    ) else {
        return;
    };
    let here = current_area(world).and_then(|area| area.ambience);
    let seconds = if music.ducked {
        PAUSE_FADE
    } else {
        AMBIENCE_FADE
    };
    let registry = world.resource::<SoundRegistry>();
    let targets: Vec<f32> = ambience
        .loops
        .iter()
        .map(|(id, ..)| {
            if music.ducked || here.as_deref() != Some(id) {
                0.0
            } else {
                registry
                    .get_by_id(id)
                    .map_or(0.0, |handle| registry.get_volume(handle))
            }
        })
        .collect();
    let mut changed = false;
    for ((_, voice, level), target) in ambience.loops.iter_mut().zip(targets) {
        if (target - *level).abs() < 1e-4 {
            continue;
        }
        world
            .resource_mut::<AudioCommands>()
            .set_voice_volume(*voice, target, seconds);
        *level = target;
        changed = true;
    }
    if changed {
        world.insert_resource(ambience);
    }
}
