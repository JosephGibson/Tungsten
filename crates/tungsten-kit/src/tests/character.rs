use super::*;
use serde_json::json;
use tungsten_core::assets::AnimationFrame;
use tungsten_core::physics::{Collider, PhysicsPlugin, RigidBodyBundle};
use tungsten_core::{AnimationData, Bundle, ComponentRegistry, SpriteAssetId, Stage, Transform};

use crate::{AnimationPlugin, PlatformerControllerPlugin, spawn_child};

/// Clips whose first frames are sprites `first`, `first + 1`, … of
/// `duration_ms` each.
fn clip(first: u32, frames: u32, duration_ms: u32) -> AnimationData {
    AnimationData {
        looping: true,
        frames: (0..frames)
            .map(|i| AnimationFrame {
                sprite: SpriteAssetId::new(first + i),
                duration_ms,
            })
            .collect(),
    }
}

fn registry() -> AnimationRegistry {
    let mut clips = AnimationRegistry::new();
    for (id, first) in [
        ("idle", 0),
        ("walk_left", 10),
        ("walk_right", 20),
        ("walk_up", 30),
        ("walk_down", 40),
        ("rise", 50),
        ("fall", 60),
    ] {
        clips.insert(id.to_string(), clip(first, 3, 100));
    }
    clips
}

fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(registry());
    let mut schedule = Schedule::new();
    PhysicsPlugin.build(&mut schedule, &mut world);
    AnimationPlugin.build(&mut schedule, &mut world);
    crate::FacingPlugin.build(&mut schedule, &mut world);
    PlatformerControllerPlugin.build(&mut schedule, &mut world);
    CharacterAnimatorPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

fn frame(world: &mut World, schedule: &mut Schedule, dt: f32) {
    world.resource_mut::<Time>().advance_frame(dt);
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
    schedule.run_stage(Stage::PostUpdate, world, |_, _| {});
}

fn shown(world: &World, entity: Entity) -> (&'static str, String, SpriteAssetId) {
    (
        world.get::<CharacterAnimator>(entity).unwrap().state(),
        world
            .get::<AnimationState>(entity)
            .unwrap()
            .animation_id
            .clone(),
        world.get::<Sprite>(entity).unwrap().asset_id,
    )
}

fn walker() -> CharacterAnimator {
    CharacterAnimator::new("idle").with_move(Clips::four(
        "walk_left",
        "walk_right",
        "walk_up",
        "walk_down",
    ))
}

#[test]
fn it_picks_idle_or_move_and_a_sets_entry_by_the_facing() {
    let (mut world, mut schedule) = rig();
    let seeker = world.spawn_with((
        walker(),
        crate::Facing::new().four_way(),
        Velocity(Vec2::ZERO),
        Sprite::new(SpriteAssetId::new(99)),
    ));
    frame(&mut world, &mut schedule, 1.0 / 60.0);
    assert_eq!(
        shown(&world, seeker),
        ("idle", "idle".into(), SpriteAssetId::new(0))
    );
    for (velocity, id, first) in [
        (Vec2::new(0.0, 40.0), "walk_down", 40),
        (Vec2::new(-40.0, 10.0), "walk_left", 10),
        (Vec2::new(5.0, -40.0), "walk_up", 30),
        (Vec2::new(40.0, 0.0), "walk_right", 20),
    ] {
        world.get_mut::<Velocity>(seeker).unwrap().0 = velocity;
        frame(&mut world, &mut schedule, 1.0 / 60.0);
        assert_eq!(
            shown(&world, seeker),
            ("move", id.into(), SpriteAssetId::new(first))
        );
    }
    world.get_mut::<Velocity>(seeker).unwrap().0 = Vec2::new(5.0, 0.0);
    frame(&mut world, &mut schedule, 1.0 / 60.0);
    assert_eq!(shown(&world, seeker).0, "idle", "under 10 px/s");
}

#[test]
fn a_sets_vertical_falls_back_to_the_last_side_and_a_missing_state_to_idle() {
    let (mut world, mut schedule) = rig();
    let runner = world.spawn_with((
        CharacterAnimator::new("idle").with_move(Clips::sides("walk_left", "walk_right")),
        Velocity(Vec2::new(-30.0, 0.0)),
        Sprite::new(SpriteAssetId::new(99)),
    ));
    frame(&mut world, &mut schedule, 1.0 / 60.0);
    assert_eq!(
        shown(&world, runner).1,
        "walk_left",
        "by its velocity's side"
    );
    world.get_mut::<Velocity>(runner).unwrap().0 = Vec2::new(0.0, 50.0);
    frame(&mut world, &mut schedule, 1.0 / 60.0);
    assert_eq!(
        shown(&world, runner).1,
        "walk_left",
        "down keeps the last side"
    );
}

#[test]
fn a_platformer_rises_and_falls_off_the_ground() {
    let (mut world, mut schedule) = rig();
    world.insert_resource(tungsten_core::PhysicsConfig {
        gravity: Vec2::new(0.0, 2000.0),
        ..tungsten_core::PhysicsConfig::default()
    });
    let mut controller = PlatformerController::new(200.0, 600.0);
    controller.launch(Vec2::new(0.0, -500.0));
    let acrobat = world.spawn_with(
        RigidBodyBundle::kinematic(Position(Vec2::ZERO), Collider::aabb(Vec2::new(10.0, 20.0)))
            .with((
                controller,
                CharacterAnimator::new("idle")
                    .with_rise("rise")
                    .with_fall("fall"),
                Sprite::new(SpriteAssetId::new(99)),
                Transform::default(),
            )),
    );
    frame(&mut world, &mut schedule, 1.0 / 60.0);
    frame(&mut world, &mut schedule, 1.0 / 60.0);
    assert_eq!(shown(&world, acrobat).0, "rise");
    for _ in 0..30 {
        frame(&mut world, &mut schedule, 1.0 / 60.0);
    }
    let (state, clip, _) = shown(&world, acrobat);
    assert_eq!((state, clip.as_str()), ("fall", "fall"), "past its peak");
}

#[test]
fn a_rider_on_a_moving_floor_idles_until_it_runs() {
    let (mut world, mut schedule) = rig();
    world.insert_resource(tungsten_core::PhysicsConfig {
        gravity: Vec2::new(0.0, 2000.0),
        ..tungsten_core::PhysicsConfig::default()
    });
    let floor = world.spawn_with(RigidBodyBundle::kinematic(
        Position(Vec2::new(0.0, 100.0)),
        Collider::aabb(Vec2::new(400.0, 10.0)),
    ));
    world.get_mut::<Velocity>(floor).unwrap().0 = Vec2::new(120.0, 0.0);
    let rider = world.spawn_with(
        RigidBodyBundle::kinematic(
            Position(Vec2::new(0.0, 69.5)),
            Collider::aabb(Vec2::new(10.0, 20.0)),
        )
        .with((
            PlatformerController::new(200.0, 600.0),
            walker(),
            Sprite::new(SpriteAssetId::new(99)),
            Transform::default(),
        )),
    );
    for _ in 0..10 {
        frame(&mut world, &mut schedule, 1.0 / 60.0);
    }
    let controller = world.get::<PlatformerController>(rider).unwrap();
    assert_eq!(controller.ground(), Some(floor), "it stands on the floor");
    let carried = world.get::<Velocity>(rider).unwrap().0;
    assert!((carried.x - 120.0).abs() < 1e-3, "carried: {carried}");
    assert_eq!(shown(&world, rider).0, "idle", "standing still on it");
    // Running against the floor slower than it moves: its own side.
    world
        .get_mut::<PlatformerController>(rider)
        .unwrap()
        .set_run(-0.5);
    frame(&mut world, &mut schedule, 1.0 / 60.0);
    frame(&mut world, &mut schedule, 1.0 / 60.0);
    let velocity = world.get::<Velocity>(rider).unwrap().0;
    assert!(velocity.x > 0.0, "still carried right: {velocity}");
    assert_eq!(
        shown(&world, rider),
        ("move", "walk_left".into(), SpriteAssetId::new(10))
    );
}

#[test]
fn a_new_clip_shows_its_first_frame_even_shorter_than_the_frame() {
    let (mut world, mut schedule) = rig();
    world
        .resource_mut::<AnimationRegistry>()
        .insert("blink".into(), clip(70, 3, 10));
    let eye = world.spawn_with((
        CharacterAnimator::new("idle")
            .with_move("blink")
            .with_min_speed(1.0),
        Velocity(Vec2::ZERO),
        Sprite::new(SpriteAssetId::new(99)),
    ));
    frame(&mut world, &mut schedule, 0.05);
    world.get_mut::<Velocity>(eye).unwrap().0 = Vec2::X * 5.0;
    frame(&mut world, &mut schedule, 0.05);
    assert_eq!(
        shown(&world, eye),
        ("move", "blink".into(), SpriteAssetId::new(70))
    );
    let state = world.get::<AnimationState>(eye).unwrap();
    assert_eq!((state.frame_index, state.accumulated_ms), (0, 0.0));
    // The next frame's tick advances it from there: 50 ms past three 10 ms
    // frames.
    frame(&mut world, &mut schedule, 0.05);
    assert_ne!(
        world.get::<Sprite>(eye).unwrap().asset_id,
        SpriteAssetId::new(70)
    );
}

#[test]
fn an_unchanged_clip_keeps_playing() {
    let (mut world, mut schedule) = rig();
    let idler = world.spawn_with((
        CharacterAnimator::new("idle"),
        Sprite::new(SpriteAssetId::new(99)),
    ));
    frame(&mut world, &mut schedule, 0.04);
    frame(&mut world, &mut schedule, 0.04);
    frame(&mut world, &mut schedule, 0.04);
    let state = world.get::<AnimationState>(idler).unwrap();
    assert_eq!(state.frame_index, 0);
    assert!(
        (state.accumulated_ms - 80.0).abs() < 1e-3,
        "{}",
        state.accumulated_ms
    );
}

#[test]
fn a_child_is_animated_and_no_registry_leaves_everything() {
    let (mut world, mut schedule) = rig();
    let parent = world.spawn_with((Transform::default(),));
    let flame = spawn_child(
        &mut world,
        parent,
        Vec2::ZERO,
        (
            CharacterAnimator::new("idle"),
            Sprite::new(SpriteAssetId::new(99)),
        ),
    );
    frame(&mut world, &mut schedule, 1.0 / 60.0);
    assert_eq!(shown(&world, flame).1, "idle");
    world.remove_resource::<AnimationRegistry>();
    let late = world.spawn_with((
        CharacterAnimator::new("idle"),
        Sprite::new(SpriteAssetId::new(99)),
    ));
    frame(&mut world, &mut schedule, 1.0 / 60.0);
    assert!(world.get::<AnimationState>(late).is_none());
}

#[test]
fn the_load_checks_every_id_naming_its_field() {
    let (world, _) = rig();
    let components = world.resource::<ComponentRegistry>();
    let good = json!({ "idle": "idle", "move": { "left": "walk_left", "right": "walk_right", "up": "walk_up" } });
    assert!(
        components
            .check_in("character_animator", &good, &world)
            .is_ok()
    );
    let bad = json!({ "idle": "idle", "move": { "left": "walk_lft", "right": "walk_right" } });
    let error = components
        .check_in("character_animator", &bad, &world)
        .unwrap_err();
    assert!(error.to_string().contains("move.left"), "{error}");
    let missing = json!({ "idle": "idle", "fall": "tumble" });
    let error = components
        .check_in("character_animator", &missing, &world)
        .unwrap_err();
    assert!(error.to_string().contains("fall"), "{error}");
    for refused in [
        json!({ "move": "walk" }),
        json!({ "idle": { "left": "a" } }),
        json!({ "idle": "a", "min_speed": -1 }),
        json!({ "idle": "a", "jump": "b" }),
    ] {
        assert!(
            components
                .check_in("character_animator", &refused, &world)
                .is_err(),
            "{refused}"
        );
    }
}

#[test]
fn a_save_keeps_its_state_and_clip() {
    let mut animator = walker().with_rise("rise").with_fall(Clips::one("fall"));
    animator.state = State::Move;
    animator.clip = Some("walk_up".to_string());
    animator.left = true;
    let saved = serde_json::to_value(&animator).unwrap();
    assert_eq!(
        serde_json::from_value::<CharacterAnimator>(saved).unwrap(),
        animator
    );
    assert_eq!(Clips::from("idle".to_string()), Clips::one("idle"));
}
