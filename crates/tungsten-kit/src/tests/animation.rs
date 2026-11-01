use super::*;
use serde_json::json;
use tungsten_core::Stage;
use tungsten_core::assets::AnimationFrame;
use tungsten_core::{
    AnimationData, CommandBuffer, ComponentRegistry, Entity, SpriteAssetId, Transform,
};

fn open() -> SpriteAssetId {
    SpriteAssetId::new(0)
}

fn shut() -> SpriteAssetId {
    SpriteAssetId::new(1)
}

fn walk(frame: u32) -> SpriteAssetId {
    SpriteAssetId::new(2 + frame)
}

fn clip(frames: &[(SpriteAssetId, u32)], looping: bool) -> AnimationData {
    AnimationData {
        looping,
        frames: frames
            .iter()
            .map(|&(sprite, duration_ms)| AnimationFrame {
                sprite,
                duration_ms,
                sound: None,
            })
            .collect(),
    }
}

/// A World with the `blink` and `walk` clips, a clock and the plugin built.
fn setup() -> (World, Schedule) {
    let mut clips = AnimationRegistry::new();
    clips.insert(
        "blink".to_string(),
        clip(&[(open(), 100), (shut(), 100)], true),
    );
    clips.insert(
        "walk".to_string(),
        clip(&[(walk(0), 50), (walk(1), 50)], true),
    );
    let mut world = World::new();
    world.insert_resource(clips);
    world.insert_resource(Time::new());
    let mut schedule = Schedule::new();
    AnimationPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

/// One frame of `dt` seconds: the clock, then `post_update`.
fn frame(world: &mut World, schedule: &mut Schedule, dt: f32) {
    world.resource_mut::<Time>().advance_frame(dt);
    schedule.run_stage(Stage::PostUpdate, world, |_, _| {});
}

fn shown(world: &World, entity: Entity) -> SpriteAssetId {
    world.get::<Sprite>(entity).unwrap().asset_id
}

#[test]
fn the_plugin_registers_its_system_in_post_update() {
    let (_, schedule) = setup();
    assert_eq!(schedule.stage_of(ANIMATION_TICK), Some(Stage::PostUpdate));
    assert_eq!(
        schedule.len(),
        9,
        "the seven anchors, the tick and the kit's despawn"
    );
}

#[test]
fn the_first_frame_shows_at_once_and_the_next_after_its_duration() {
    let (mut world, mut schedule) = setup();
    let eye = world.spawn_with((AnimationState::new("blink"), Sprite::new(shut())));
    frame(&mut world, &mut schedule, 0.0);
    assert_eq!(shown(&world, eye), open());
    frame(&mut world, &mut schedule, 0.05);
    assert_eq!(shown(&world, eye), open());
    frame(&mut world, &mut schedule, 0.05);
    assert_eq!(shown(&world, eye), shut());
    // A looping clip comes back to its first frame.
    frame(&mut world, &mut schedule, 0.1);
    assert_eq!(shown(&world, eye), open());
}

#[test]
fn a_paused_clock_holds_the_frame() {
    let (mut world, mut schedule) = setup();
    let eye = world.spawn_with((AnimationState::new("blink"), Sprite::new(open())));
    world.resource_mut::<Time>().pause();
    for _ in 0..10 {
        frame(&mut world, &mut schedule, 0.1);
    }
    assert_eq!(shown(&world, eye), open());
    assert_eq!(world.get::<AnimationState>(eye).unwrap().frame_index, 0);
    world.resource_mut::<Time>().set_paused(false);
    frame(&mut world, &mut schedule, 0.1);
    assert_eq!(shown(&world, eye), shut());
}

#[test]
fn a_swapped_clip_shows_on_the_next_frame() {
    let (mut world, mut schedule) = setup();
    let hero = world.spawn_with((AnimationState::new("blink"), Sprite::new(open())));
    frame(&mut world, &mut schedule, 0.0);
    *world.get_mut::<AnimationState>(hero).unwrap() = AnimationState::new("walk");
    frame(&mut world, &mut schedule, 0.0);
    assert_eq!(shown(&world, hero), walk(0));
    frame(&mut world, &mut schedule, 0.05);
    assert_eq!(shown(&world, hero), walk(1));
}

#[test]
fn an_unknown_animation_leaves_the_sprite_alone() {
    let (mut world, mut schedule) = setup();
    let ghost = world.spawn_with((AnimationState::new("missing"), Sprite::new(shut())));
    frame(&mut world, &mut schedule, 0.5);
    assert_eq!(shown(&world, ghost), shut());
}

#[test]
fn without_a_registry_the_system_does_nothing() {
    let (mut world, mut schedule) = setup();
    world.remove_resource::<AnimationRegistry>();
    let eye = world.spawn_with((AnimationState::new("blink"), Sprite::new(shut())));
    frame(&mut world, &mut schedule, 0.5);
    assert_eq!(shown(&world, eye), shut());
    assert_eq!(world.get::<AnimationState>(eye).unwrap().frame_index, 0);
    assert!(!world.has_resource::<AnimationRegistry>());
}

#[test]
fn the_registry_goes_back_into_the_world() {
    let (mut world, mut schedule) = setup();
    world.spawn_with((AnimationState::new("blink"), Sprite::new(shut())));
    frame(&mut world, &mut schedule, 0.1);
    assert_eq!(world.resource::<AnimationRegistry>().ids().count(), 2);
}

#[test]
fn an_entity_without_a_sprite_is_not_advanced() {
    let (mut world, mut schedule) = setup();
    let state_only = world.spawn_with((AnimationState::new("blink"),));
    frame(&mut world, &mut schedule, 0.1);
    assert_eq!(
        world.get::<AnimationState>(state_only).unwrap().frame_index,
        0
    );
}

#[test]
fn the_animation_scene_component_spawns_a_state() {
    let (mut world, _) = setup();
    let registry = world.resource::<ComponentRegistry>();
    assert!(registry.check(ANIMATION, &json!({ "id": "walk" })).is_ok());
    // Redo 7.9: `speed` and `despawn` are fields; another key is unknown.
    assert!(
        registry
            .check(
                ANIMATION,
                &json!({ "id": "walk", "speed": 2, "despawn": true })
            )
            .is_ok()
    );
    let unknown_key = registry
        .check(ANIMATION, &json!({ "id": "walk", "pace": 2 }))
        .unwrap_err();
    assert!(unknown_key.to_string().contains("pace"), "{unknown_key}");
    assert!(registry.check(ANIMATION, &json!({})).is_err());

    let prepared = registry
        .prepare(
            ANIMATION,
            &json!({ "id": "walk" }),
            &SpawnContext::new(&world, Transform::default()),
        )
        .unwrap();
    let mut buffer = CommandBuffer::new();
    let pending = buffer.spawn();
    prepared.insert_pending(&mut buffer, pending);
    world.flush_reusing(&mut buffer);
    let states: Vec<&AnimationState> = world.query::<&AnimationState>().collect();
    assert_eq!(states.len(), 1);
    assert_eq!(states[0].animation_id, "walk");
    assert_eq!(states[0].frame_index, 0);
}

// Redo 7.9: rates, one-shot ends and frame sounds.

/// A clip whose frames name sounds: `(sprite, duration, sound)`.
fn sounded(frames: &[(SpriteAssetId, u32, Option<&str>)], looping: bool) -> AnimationData {
    AnimationData {
        looping,
        frames: frames
            .iter()
            .map(|&(sprite, duration_ms, sound)| AnimationFrame {
                sprite,
                duration_ms,
                sound: sound.map(str::to_string),
            })
            .collect(),
    }
}

/// `setup`'s World with the `step` (0.5) and `clang` (0.8) sounds, the
/// queue, the kit's despawn list and the `stomp` clip: three 100 ms
/// frames, the second and the third sounded, looping.
fn sound_setup() -> (World, Schedule) {
    let (mut world, schedule) = setup();
    let mut sounds = tungsten_core::SoundRegistry::new();
    let data = || tungsten_core::SoundData {
        samples: vec![0.0; 4],
        sample_rate: 44_100,
        channels: 1,
    };
    sounds.register("step".to_string(), data(), 0.5, false);
    sounds.register("clang".to_string(), data(), 0.8, false);
    world.insert_resource(sounds);
    world.insert_resource(tungsten_core::AudioCommands::new());
    world.resource_mut::<AnimationRegistry>().insert(
        "stomp".to_string(),
        sounded(
            &[
                (walk(0), 100, None),
                (walk(1), 100, Some("step")),
                (walk(2), 100, Some("clang")),
            ],
            true,
        ),
    );
    (world, schedule)
}

/// The volumes of the one-shots the frame played.
fn played(world: &mut World) -> Vec<f32> {
    world
        .resource_mut::<tungsten_core::AudioCommands>()
        .drain()
        .into_iter()
        .filter_map(|command| match command {
            tungsten_core::AudioCommand::Play {
                volume,
                looping: false,
                ..
            } => Some(volume),
            _ => None,
        })
        .collect()
}

#[test]
fn a_clip_at_half_speed_takes_twice_as_long() {
    let (mut world, mut schedule) = setup();
    let eye = world.spawn_with((
        AnimationState::new("blink").with_speed(0.5),
        Sprite::new(shut()),
    ));
    frame(&mut world, &mut schedule, 0.0);
    frame(&mut world, &mut schedule, 0.15);
    assert_eq!(shown(&world, eye), open(), "75 ms of clip in 150 ms");
    frame(&mut world, &mut schedule, 0.05);
    assert_eq!(
        shown(&world, eye),
        shut(),
        "the 100 ms frame ends at 200 ms"
    );
    // The scene's `speed`.
    let def: AnimationDef = serde_json::from_value(json!({ "id": "blink", "speed": 0.5 })).unwrap();
    assert_eq!(def.speed, 0.5);
    let plain: AnimationDef = serde_json::from_value(json!({ "id": "blink" })).unwrap();
    assert_eq!((plain.speed, plain.despawn), (1.0, false));
}

#[test]
fn a_speed_survives_a_clip_change() {
    use tungsten_core::Velocity;
    let (mut world, mut schedule) = setup();
    let mut schedule2 = Schedule::new();
    crate::CharacterAnimatorPlugin.build(&mut schedule2, &mut world);
    schedule2.resolve().unwrap();
    let hero = world.spawn_with((
        crate::CharacterAnimator::new("blink").with_move(crate::Clips::one("walk")),
        AnimationState::new("blink").with_speed(0.25),
        Sprite::new(open()),
        Velocity(glam::Vec2::new(200.0, 0.0)),
        Transform::default(),
    ));
    // The animator picks `walk` for a moving entity and keeps its speed.
    schedule2.run_stage(Stage::PostUpdate, &mut world, |_, _| {});
    let state = world.get::<AnimationState>(hero).unwrap();
    assert_eq!(state.animation_id, "walk");
    assert_eq!(state.speed(), 0.25);
    frame(&mut world, &mut schedule, 0.1);
    assert_eq!(shown(&world, hero), walk(0), "25 ms of a 50 ms frame");
}

#[test]
fn a_one_shot_that_despawns_ends_its_entity_after_its_last_frame() {
    let (mut world, mut schedule) = setup();
    world.insert_resource(CommandBuffer::new());
    world.resource_mut::<AnimationRegistry>().insert(
        "puff".to_string(),
        clip(&[(walk(0), 50), (walk(1), 50)], false),
    );
    let puff = world.spawn_with((
        AnimationState::new("puff").with_despawn_at_end(true),
        Sprite::new(walk(0)),
    ));
    frame(&mut world, &mut schedule, 0.0);
    frame(&mut world, &mut schedule, 0.06);
    let step = |world: &mut World, schedule: &mut Schedule| {
        schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
        let commands = world.remove_resource::<CommandBuffer>().unwrap();
        world.flush(commands);
        world.insert_resource(CommandBuffer::new());
    };
    step(&mut world, &mut schedule);
    assert!(world.is_alive(puff), "its last frame still shows");
    frame(&mut world, &mut schedule, 0.05);
    assert!(world.get::<AnimationState>(puff).unwrap().finished);
    step(&mut world, &mut schedule);
    assert!(
        !world.is_alive(puff),
        "the kit's despawn ends it after its last frame"
    );
}

#[test]
fn a_looping_clip_never_despawns() {
    let (mut world, mut schedule) = setup();
    world.insert_resource(CommandBuffer::new());
    let eye = world.spawn_with((
        AnimationState::new("blink").with_despawn_at_end(true),
        Sprite::new(open()),
    ));
    for _ in 0..30 {
        frame(&mut world, &mut schedule, 0.05);
        schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
        let commands = world.remove_resource::<CommandBuffer>().unwrap();
        world.flush(commands);
        world.insert_resource(CommandBuffer::new());
    }
    assert!(world.is_alive(eye));
    assert!(!world.get::<AnimationState>(eye).unwrap().finished);
}

#[test]
fn a_frame_sound_plays_when_its_frame_starts() {
    let (mut world, mut schedule) = sound_setup();
    world.spawn_with((AnimationState::new("stomp"), Sprite::new(walk(0))));
    frame(&mut world, &mut schedule, 0.0);
    assert!(played(&mut world).is_empty(), "the first frame is silent");
    frame(&mut world, &mut schedule, 0.09);
    assert!(played(&mut world).is_empty());
    frame(&mut world, &mut schedule, 0.02);
    assert_eq!(
        played(&mut world),
        [0.5],
        "`step` as the second frame starts"
    );
    frame(&mut world, &mut schedule, 0.05);
    assert!(played(&mut world).is_empty(), "once a frame");
    frame(&mut world, &mut schedule, 0.05);
    assert_eq!(played(&mut world), [0.8], "`clang` on the third");
}

#[test]
fn one_tick_over_many_frames_plays_one_sound() {
    let (mut world, mut schedule) = sound_setup();
    world.spawn_with((AnimationState::new("stomp"), Sprite::new(walk(0))));
    frame(&mut world, &mut schedule, 0.0);
    // One 250 ms tick enters the second frame and the third.
    frame(&mut world, &mut schedule, 0.25);
    assert_eq!(
        played(&mut world),
        [0.8],
        "the last entered frame's sound alone"
    );
}

#[test]
fn a_frame_sound_takes_the_listeners_gain() {
    let (mut world, mut schedule) = sound_setup();
    world.insert_resource(CueListener::new(100.0, 300.0));
    world.spawn_with((
        AnimationState::new("stomp"),
        Sprite::new(walk(0)),
        Transform::from_position(glam::Vec2::new(200.0, 0.0)),
    ));
    let far = world.spawn_with((
        AnimationState::new("stomp"),
        Sprite::new(walk(0)),
        Transform::from_position(glam::Vec2::new(400.0, 0.0)),
    ));
    frame(&mut world, &mut schedule, 0.0);
    frame(&mut world, &mut schedule, 0.11);
    // (1 − 0.5)² of 0.5; the far one is past hearing.
    assert_eq!(played(&mut world), [0.125]);
    assert!(world.is_alive(far));
}
