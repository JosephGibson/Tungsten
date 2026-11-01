//! The animated sprite: [`AnimationPlugin`] drives each entity's
//! `AnimationState` and shows its current frame through the entity's
//! `Sprite`, which the default extract draws.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::{
    AnimationRegistry, AnimationState, AudioCommands, Entity, Plugin, Schedule, SoundRegistry,
    SpawnContext, Sprite, Time, Transform, World, system,
};

use crate::foundation::describe::{
    ComponentDescription, FieldDescription, FieldType, PLATFORMER, TOP_DOWN, describe, round_trip,
};
use crate::foundation::inspect::register_rows_with;
use crate::foundation::phases::{Phase, ensure_phases};
use crate::lifecycle::ended::{end, ensure_kit_despawn};
use crate::react::cues::CueListener;

/// [`AnimationPlugin`]'s system, in the present phase.
pub const ANIMATION_TICK: &str = "animation_tick";

/// The scene component the plugin registers.
const ANIMATION: &str = "animation";

/// The animated sprite. Each frame its system, [`ANIMATION_TICK`], advances
/// every entity that has an [`AnimationState`] and a [`Sprite`] by the game
/// clock's dt ([`Time::delta`]: a pause holds it, the time scale stretches
/// it) against the [`AnimationRegistry`] resource, then writes the current
/// frame's sprite into `Sprite.asset_id` when the two differ. So the first
/// frame shows from the entity's first frame, and a clip the game swaps in
/// shows on the next. Without a registry the system does nothing, and an
/// unknown animation ID leaves the sprite as it is.
///
/// The plugin also registers the `animation` scene component,
/// `{ "id": "<animation id>" }`, which spawns `AnimationState::new(id)`; an
/// unknown key fails the scene.
///
/// # Examples
///
/// ```
/// use tungsten_core::assets::AnimationFrame;
/// use tungsten_core::{
///     AnimationData, AnimationRegistry, AnimationState, Plugin, Schedule, Sprite,
///     SpriteAssetId, Stage, Time, World,
/// };
/// use tungsten_kit::AnimationPlugin;
///
/// let (open, shut) = (SpriteAssetId::new(0), SpriteAssetId::new(1));
/// let mut clips = AnimationRegistry::new();
/// clips.insert(
///     "blink".to_string(),
///     AnimationData {
///         looping: true,
///         frames: vec![
///             AnimationFrame { sprite: open, duration_ms: 100, sound: None },
///             AnimationFrame { sprite: shut, duration_ms: 100, sound: None },
///         ],
///     },
/// );
/// let mut world = World::new();
/// world.insert_resource(clips);
/// world.insert_resource(Time::new());
/// let mut schedule = Schedule::new();
/// AnimationPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// let eye = world.spawn_with((AnimationState::new("blink"), Sprite::new(shut)));
/// // The first frame shows at once, the second 0.1 s of game time later.
/// for shown in [open, shut] {
///     schedule.run_stage(Stage::PostUpdate, &mut world, |_, _| {});
///     assert_eq!(world.get::<Sprite>(eye).unwrap().asset_id, shown);
///     world.resource_mut::<Time>().advance_frame(0.1);
/// }
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct AnimationPlugin;

impl Plugin for AnimationPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        ensure_kit_despawn(schedule, world);
        register_rows_with(world, "AnimationState", |world: &World, entity: Entity| {
            world
                .get::<AnimationState>(entity)
                .map(|state| {
                    vec![
                        ("animation", state.animation_id.clone()),
                        ("frame", state.frame_index.to_string()),
                    ]
                })
                .unwrap_or_default()
        });
        describe(
            world,
            ComponentDescription::new(
                ANIMATION,
                "An animated sprite: its current frame written into the entity's Sprite; \
                 optional `speed` (its rate, 1 unless set) and `despawn` (a finished \
                 one-shot ends the entity).",
            )
            .field(FieldDescription::new(
                "id",
                FieldType::Text,
                "a manifest animation ID",
            ))
            .genre(PLATFORMER)
            .genre(TOP_DOWN)
            .sample(r#"{ "id": "blink" }"#)
            .roundtrip(round_trip::<AnimationDef>),
        );
        world.register_component_with(ANIMATION, |def: AnimationDef, _: &SpawnContext<'_>| {
            (AnimationState::new(def.id)
                .with_speed(def.speed)
                .with_despawn_at_end(def.despawn),)
        });
        schedule.add(
            Phase::Present.stage(),
            Phase::Present.join(system(ANIMATION_TICK, animation_tick)),
        );
    }
}

/// `components.animation` as a scene writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AnimationDef {
    id: String,
    /// Redo 7.9: the rate, 1 unless set.
    #[serde(default = "one")]
    speed: f32,
    /// Redo 7.9: a finished one-shot ends the entity.
    #[serde(default)]
    despawn: bool,
}

fn one() -> f32 {
    1.0
}

/// Advances every animated sprite at its speed and shows its current frame
/// (redo 7.9: plays the sound of the last frame each advance entered, and
/// ends an entity whose one-shot finished with `despawn` set). The registry
/// leaves the World for the pass and goes back after it, never cloned.
fn animation_tick(world: &mut World) {
    let dt_ms = world.get_resource::<Time>().map_or(0.0, Time::delta) * 1000.0;
    let Some(registry) = world.remove_resource::<AnimationRegistry>() else {
        return;
    };
    // The pass's query is the two columns it writes, as before redo 7.9:
    // each further element allocates per call, which example 01's guard
    // pins. A frame that sounds or ends a one-shot reads its entities after.
    let mut sounds: Vec<(usize, String)> = Vec::new();
    let mut finishing = false;
    for (row, (state, sprite)) in world
        .query_mut::<(&mut AnimationState, &mut Sprite)>()
        .enumerate()
    {
        let was_finished = state.finished;
        let advanced = state.advance_entering(dt_ms, &registry);
        if let Some(frame) = advanced.sound_frame
            && let Some(sound) = registry
                .get(&state.animation_id)
                .and_then(|clip| clip.frames[frame].sound.as_ref())
        {
            sounds.push((row, sound.clone()));
        }
        finishing |= state.finished && !was_finished && state.despawn_at_end();
        if let Some(current) = state.current_sprite(&registry)
            && current != sprite.asset_id
        {
            sprite.asset_id = current;
        }
    }
    world.insert_resource(registry);
    if sounds.is_empty() && !finishing {
        return;
    }
    // The pass's rows in its order, each entity and whether it ends; `end`
    // queues an entity once, however often it is listed.
    let rows: Vec<(Entity, bool)> = world
        .query::<(Entity, &AnimationState, &Sprite)>()
        .map(|(entity, state, _)| (entity, state.finished && state.despawn_at_end()))
        .collect();
    for (row, sound) in sounds {
        let entity = rows[row].0;
        let at = world.get::<Transform>(entity).map(|t| t.position);
        play_frame_sound(world, &sound, at);
    }
    if finishing {
        for &(entity, ends) in &rows {
            if ends {
                end(world, entity, ANIMATION);
            }
        }
    }
}

/// A frame's sound, at its manifest volume × the cue listener's gain at the
/// entity's position, as a `sound` cue plays; past the listener's hearing,
/// or without a position, at the volume alone.
fn play_frame_sound(world: &mut World, id: &str, at: Option<Vec2>) {
    let Some((handle, volume)) = world
        .get_resource::<SoundRegistry>()
        .and_then(|sounds| sounds.get_by_id(id).map(|h| (h, sounds.get_volume(h))))
    else {
        return;
    };
    let gain = match (world.get_resource::<CueListener>(), at) {
        (Some(listener), Some(at)) => listener.gain(at),
        _ => 1.0,
    };
    if gain <= 0.0 {
        return;
    }
    if let Some(audio) = world.get_resource_mut::<AudioCommands>() {
        audio.play_with(handle, volume * gain, false);
    }
}

#[cfg(test)]
#[path = "../tests/animation.rs"]
mod tests;
