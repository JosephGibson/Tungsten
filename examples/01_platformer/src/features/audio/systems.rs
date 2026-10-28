//! The audio's systems.

use glam::Vec2;
use tungsten::core::{AudioCommands, Entity, SoundRegistry, World};
use tungsten::kit::{CueListener, Health, StateMachine, Tags};

use super::components::{Ambience, Music};
use super::{
    AMBIENCE_FADE, COMBAT_IN, COMBAT_OUT, FIRE_CRACKLE, HOLE_HUM, HUM_FADE, MUSIC_COMBAT,
    MUSIC_EXPLORE, MUSIC_PAUSE, PAUSE_FADE, WISP_HUM,
};
use crate::features::combat::BURNING;
use crate::features::elements::components::BallBurn;
use crate::features::enemies;
use crate::features::enemies::components::{Enemy, EnemyKind, EnemyState, Sight};
use crate::features::level::systems::current_area;
use crate::features::props::components::Fuel;
use crate::features::toys;
use crate::game::GAMEPLAY;
use crate::shared::body_centre;
use crate::shell::game_over::GAME_OVER;
use crate::shell::pause::PAUSE;
use crate::shell::{self};

/// `audio_duck`: when pause comes on top, the exploration loop falls to 0
/// and pause's loop fades in; when gameplay is back on top, pause's loop
/// stops and the exploration comes back. A screen over pause keeps the
/// loop. Game over silences the exploration too, under the music the shell
/// plays, and the hums and the crackle fall silent until gameplay is back.
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
    let mut holes_level = music.holes_level;
    if let (Some(holes), true) = (music.holes, on) {
        audio.set_voice_volume(holes, 0.0, PAUSE_FADE);
        holes_level = 0.0;
    }
    let mut crackle_level = music.crackle_level;
    if let (Some(crackle), true) = (music.crackle, on) {
        audio.set_voice_volume(crackle, 0.0, PAUSE_FADE);
        crackle_level = 0.0;
    }
    world.insert_resource(Music {
        pause,
        hum_level,
        holes_level,
        crackle_level,
        ducked: on,
        ..music
    });
}

/// `audio_listen`, in the react phase before the kit's cues: the listener
/// follows the live player's centre, so the step's sounds are placed by
/// it; a dead player's stays where it fell.
pub fn listen(world: &mut World) {
    let Some((_, at)) = enemies::systems::player(world) else {
        return;
    };
    if let Some(listener) = world.get_resource_mut::<CueListener>() {
        listener.set_position(at);
    }
}

/// The listener's gain at the nearest of `sources`, the one it hears
/// loudest: 0 with no listener or no source.
fn nearest_gain(world: &World, sources: impl IntoIterator<Item = Vec2>) -> f32 {
    world.get_resource::<CueListener>().map_or(0.0, |listener| {
        sources
            .into_iter()
            .map(|at| listener.gain(at))
            .fold(0.0, f32::max)
    })
}

/// `audio_hum`, after the duck: while gameplay is on top, the wisps' hum
/// sounds at its level times the listener's gain at the nearest live,
/// thinking wisp.
pub fn hum(world: &mut World) {
    let Some(music) = world.get_resource::<Music>().copied() else {
        return;
    };
    let Some(voice) = music.hum.filter(|_| !music.ducked) else {
        return;
    };
    let wisps = world
        .query::<(Entity, &Enemy, &Health, &StateMachine<EnemyState>)>()
        .filter(|(_, enemy, health, machine)| {
            enemy.kind == EnemyKind::Wisp && !health.is_dead() && !machine.is_held()
        })
        .filter_map(|(entity, ..)| enemies::centre(world, entity));
    let gain = nearest_gain(world, wisps);
    let registry = world.resource::<SoundRegistry>();
    let level = registry
        .get_by_id(WISP_HUM)
        .map_or(0.0, |handle| registry.get_volume(handle));
    let volume = level * gain;
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

/// `audio_holes`, after the duck: while gameplay is on top, the holes' hum
/// sounds at its level times the listener's gain at the nearest open hole,
/// falling to 0 when the last hole closes.
pub fn holes(world: &mut World) {
    let Some(music) = world.get_resource::<Music>().copied() else {
        return;
    };
    let Some(voice) = music.holes.filter(|_| !music.ducked) else {
        return;
    };
    let gain = nearest_gain(world, toys::systems::holes(world));
    let registry = world.resource::<SoundRegistry>();
    let level = registry
        .get_by_id(HOLE_HUM)
        .map_or(0.0, |handle| registry.get_volume(handle));
    let volume = level * gain;
    if (volume - music.holes_level).abs() < 0.01 && (volume > 0.0 || music.holes_level == 0.0) {
        return;
    }
    world
        .resource_mut::<AudioCommands>()
        .set_voice_volume(voice, volume, HUM_FADE);
    world.insert_resource(Music {
        holes_level: volume,
        ..music
    });
}

/// Every burning thing's centre: lit fuel (the crates), the small balls
/// with fire left, and the live targets tagged burning (the player, the
/// dummy and the enemies).
fn fires(world: &World) -> impl Iterator<Item = Vec2> + '_ {
    let fuel = world
        .query::<(Entity, &Fuel)>()
        .filter(|(_, fuel)| fuel.lit)
        .map(|(entity, _)| entity);
    let balls = world
        .query::<(Entity, &BallBurn)>()
        .filter(|(_, burn)| burn.remaining > 0.0)
        .map(|(entity, _)| entity);
    let targets = world
        .query::<(Entity, &Health, &Tags)>()
        .filter(|(_, health, tags)| !health.is_dead() && tags.has(BURNING))
        .map(|(entity, ..)| entity);
    fuel.chain(balls)
        .chain(targets)
        .filter_map(|entity| enemies::centre(world, entity).or_else(|| body_centre(world, entity)))
}

/// `audio_crackle`, after the duck: while gameplay is on top, the fires'
/// crackle sounds at its level times the listener's gain at the nearest
/// burning thing, falling to 0 when the last fire falls in or goes out.
pub fn crackle(world: &mut World) {
    let Some(music) = world.get_resource::<Music>().copied() else {
        return;
    };
    let Some(voice) = music.crackle.filter(|_| !music.ducked) else {
        return;
    };
    let gain = nearest_gain(world, fires(world));
    let registry = world.resource::<SoundRegistry>();
    let level = registry
        .get_by_id(FIRE_CRACKLE)
        .map_or(0.0, |handle| registry.get_volume(handle));
    let volume = level * gain;
    if (volume - music.crackle_level).abs() < 0.01 && (volume > 0.0 || music.crackle_level == 0.0) {
        return;
    }
    world
        .resource_mut::<AudioCommands>()
        .set_voice_volume(voice, volume, HUM_FADE);
    world.insert_resource(Music {
        crackle_level: volume,
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
