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
    assert_eq!(schedule.len(), 8, "the seven anchors and the tick");
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
    let unknown_key = registry
        .check(ANIMATION, &json!({ "id": "walk", "speed": 2 }))
        .unwrap_err();
    assert!(unknown_key.to_string().contains("speed"), "{unknown_key}");
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
