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
                sound: None,
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
        ("land", 70),
        ("alert", 80),
        ("charge", 90),
        ("aim_down", 110),
        ("aim_fwd", 120),
        ("aim_up", 130),
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

#[test]
fn a_clip_set_without_left_shows_the_right_flipped() {
    use tungsten_core::components::SpriteFlip;
    let (mut world, mut schedule) = rig();
    let set: Clips = serde_json::from_value(json!({ "right": "walk_right" })).unwrap();
    assert_eq!(set, Clips::mirrored("walk_right"));
    let runner = world.spawn_with((
        CharacterAnimator::new("idle").with_move(set),
        Velocity(Vec2::new(40.0, 0.0)),
        Sprite::new(SpriteAssetId::new(99)),
    ));
    frame(&mut world, &mut schedule, 1.0 / 60.0);
    assert_eq!(
        shown(&world, runner),
        ("move", "walk_right".into(), SpriteAssetId::new(20))
    );
    assert!(
        world.get::<SpriteFlip>(runner).is_none(),
        "facing right, nothing to flip yet"
    );
    // Facing left: the same clip, flipped, with no change of clip.
    let playing = world.get::<AnimationState>(runner).unwrap().clone();
    world.get_mut::<Velocity>(runner).unwrap().0 = Vec2::new(-40.0, 0.0);
    frame(&mut world, &mut schedule, 1.0 / 60.0);
    assert_eq!(shown(&world, runner).1, "walk_right");
    assert_eq!(world.get::<SpriteFlip>(runner), Some(&SpriteFlip(true)));
    assert_eq!(
        world.get::<AnimationState>(runner).unwrap().frame_index,
        playing.frame_index,
        "the clip plays on"
    );
    // Stopped, the one-clip idle keeps the side it faced.
    world.get_mut::<Velocity>(runner).unwrap().0 = Vec2::ZERO;
    frame(&mut world, &mut schedule, 1.0 / 60.0);
    assert_eq!(shown(&world, runner).1, "idle");
    assert_eq!(world.get::<SpriteFlip>(runner), Some(&SpriteFlip(true)));
    world.get_mut::<Velocity>(runner).unwrap().0 = Vec2::new(40.0, 0.0);
    frame(&mut world, &mut schedule, 1.0 / 60.0);
    assert_eq!(world.get::<SpriteFlip>(runner), Some(&SpriteFlip(false)));
    // A set with both sides never writes a flip.
    let walker = world.spawn_with((
        CharacterAnimator::new("idle").with_move(Clips::sides("walk_left", "walk_right")),
        Velocity(Vec2::new(-40.0, 0.0)),
        Sprite::new(SpriteAssetId::new(99)),
    ));
    frame(&mut world, &mut schedule, 1.0 / 60.0);
    assert_eq!(shown(&world, walker).1, "walk_left");
    assert!(world.get::<SpriteFlip>(walker).is_none());
    // The load checks `right` alone, and a save keeps the set as written.
    assert_eq!(
        serde_json::to_value(Clips::mirrored("walk_right")).unwrap(),
        json!({ "right": "walk_right" })
    );
}

/// The one-shot `land` (three frames of 100 ms) over a runner's move: shown
/// from its first frame for its 300 ms, 18 frames at 1/60 s and 9 at 1/30,
/// then the move clip from its first frame.
#[test]
fn a_one_shot_plays_once_over_the_state_and_hands_back() {
    for (dt, frames) in [(1.0 / 60.0, 18), (1.0 / 30.0, 9)] {
        let (mut world, mut schedule) = rig();
        let runner = world.spawn_with((
            walker(),
            Velocity(Vec2::new(40.0, 0.0)),
            Sprite::new(SpriteAssetId::new(99)),
        ));
        frame(&mut world, &mut schedule, dt);
        assert_eq!(shown(&world, runner).1, "walk_right");
        world
            .get_mut::<CharacterAnimator>(runner)
            .unwrap()
            .play("land");
        assert_eq!(
            world.get::<CharacterAnimator>(runner).unwrap().playing(),
            Some(&Clips::one("land"))
        );
        for i in 0..frames {
            frame(&mut world, &mut schedule, dt);
            let (state, clip, sprite) = shown(&world, runner);
            assert_eq!(
                (state, clip.as_str()),
                ("move", "land"),
                "dt {dt}, frame {i}: the state runs on beneath it"
            );
            if i == 0 {
                assert_eq!(sprite, SpriteAssetId::new(70), "from its first frame");
            }
        }
        frame(&mut world, &mut schedule, dt);
        assert_eq!(
            shown(&world, runner),
            ("move", "walk_right".into(), SpriteAssetId::new(20)),
            "dt {dt}: the move clip again, from its first frame"
        );
        assert_eq!(world.get::<AnimationState>(runner).unwrap().frame_index, 0);
        assert!(
            world
                .get::<CharacterAnimator>(runner)
                .unwrap()
                .playing()
                .is_none()
        );
    }
}

#[test]
fn a_one_shot_restarts_and_holds_through_a_pause() {
    let dt = 1.0 / 60.0;
    let (mut world, mut schedule) = rig();
    let idler = world.spawn_with((walker(), Sprite::new(SpriteAssetId::new(99))));
    frame(&mut world, &mut schedule, dt);
    let play = |world: &mut World, id: &str| {
        world.get_mut::<CharacterAnimator>(idler).unwrap().play(id);
    };
    play(&mut world, "land");
    for _ in 0..8 {
        frame(&mut world, &mut schedule, dt);
    }
    assert_eq!(
        shown(&world, idler).2,
        SpriteAssetId::new(71),
        "past its first 100 ms frame"
    );
    // The same clip again: from its first frame, its time from 0.
    play(&mut world, "land");
    frame(&mut world, &mut schedule, dt);
    assert_eq!(shown(&world, idler).2, SpriteAssetId::new(70));
    assert_eq!(world.get::<AnimationState>(idler).unwrap().frame_index, 0);
    for _ in 0..9 {
        frame(&mut world, &mut schedule, dt);
    }
    // Paused, the game clock stands: the one-shot holds its frame.
    world.resource_mut::<Time>().pause();
    let held = shown(&world, idler);
    for _ in 0..120 {
        frame(&mut world, &mut schedule, dt);
    }
    assert_eq!(shown(&world, idler), held, "held through the pause");
    world.resource_mut::<Time>().resume();
    // 150 ms shown before the pause: 150 more, at 1/60 s, is 9 frames.
    for i in 0..8 {
        frame(&mut world, &mut schedule, dt);
        assert_eq!(shown(&world, idler).1, "land", "frame {i}");
    }
    frame(&mut world, &mut schedule, dt);
    assert_eq!(
        shown(&world, idler),
        ("idle", "idle".into(), SpriteAssetId::new(0))
    );
    // A new clip mid-way replaces it; `stop` hands back at once.
    play(&mut world, "land");
    frame(&mut world, &mut schedule, dt);
    play(&mut world, "alert");
    frame(&mut world, &mut schedule, dt);
    assert_eq!(shown(&world, idler).1, "alert");
    world.get_mut::<CharacterAnimator>(idler).unwrap().stop();
    frame(&mut world, &mut schedule, dt);
    assert_eq!(
        shown(&world, idler),
        ("idle", "idle".into(), SpriteAssetId::new(0))
    );
}

#[test]
fn the_first_passing_rule_picks_by_tag_and_motion() {
    use crate::{ClipRule, TagFilter, Tags, TagsPlugin};
    let dt = 1.0 / 60.0;
    let (mut world, mut schedule) = rig();
    let alerted = || TagFilter::new().all(["state.alerted"]);
    let animator = walker()
        .with_rule(ClipRule::when(
            TagFilter::new().all(["state.charging"]),
            "charge",
        ))
        .with_rule(ClipRule::when(alerted(), "alert").with_motion("move"))
        .with_rule(ClipRule::motion("idle", "rise"));
    let ghoul = world.spawn_with((
        animator,
        Tags::new(Vec::<String>::new()),
        Velocity(Vec2::ZERO),
        Sprite::new(SpriteAssetId::new(99)),
    ));
    let set = |world: &mut World, tags: &[&str], velocity: Vec2| {
        *world.get_mut::<Tags>(ghoul).unwrap() = Tags::new(tags.iter().copied());
        world.get_mut::<Velocity>(ghoul).unwrap().0 = velocity;
    };
    let run = Vec2::new(40.0, 0.0);
    for (tags, velocity, want) in [
        (&[][..], Vec2::ZERO, "rise"),
        (&[], run, "walk_right"),
        (&["state.alerted"], run, "alert"),
        (&["state.alerted", "state.charging"], run, "charge"),
        (&["state.alerted"], Vec2::ZERO, "rise"),
    ] {
        set(&mut world, tags, velocity);
        frame(&mut world, &mut schedule, dt);
        assert_eq!(shown(&world, ghoul).1, want, "{tags:?} at {velocity}");
    }
    // A one-shot wins over every rule.
    set(&mut world, &["state.charging"], run);
    world
        .get_mut::<CharacterAnimator>(ghoul)
        .unwrap()
        .play("land");
    frame(&mut world, &mut schedule, dt);
    assert_eq!(shown(&world, ghoul).1, "land");

    // The load checks each rule's tags and IDs, and needs `when` or
    // `motion`.
    TagsPlugin::new()
        .with_tags(["state.alerted"])
        .build(&mut schedule, &mut world);
    let components = world.resource::<ComponentRegistry>();
    let rules = |rule: serde_json::Value| json!({ "idle": "idle", "rules": [rule] });
    let good =
        rules(json!({ "when": { "all": ["state.alerted"] }, "motion": "move", "clip": "alert" }));
    components
        .check_in("character_animator", &good, &world)
        .unwrap();
    for (bad, field) in [
        (
            rules(json!({ "when": { "all": ["state.dazed"] }, "clip": "alert" })),
            "rules.0.when.all.0",
        ),
        (
            rules(json!({ "motion": "move", "clip": "alret" })),
            "rules.0.clip",
        ),
        (
            rules(json!({ "motion": "move", "clip": { "aim": ["aim_down", "aim_upp"] } })),
            "rules.0.clip.aim.1",
        ),
    ] {
        let error = components
            .check_in("character_animator", &bad, &world)
            .unwrap_err();
        assert!(error.to_string().contains(field), "{error}");
    }
    for refused in [
        rules(json!({ "clip": "alert" })),
        rules(json!({ "motion": "dash", "clip": "alert" })),
        rules(json!({ "motion": "move", "clip": "alert", "then": "idle" })),
    ] {
        assert!(
            components
                .check_in("character_animator", &refused, &world)
                .is_err(),
            "{refused}"
        );
    }
    // A save keeps the rules.
    let saved = serde_json::to_value(world.get::<CharacterAnimator>(ghoul).unwrap()).unwrap();
    let loaded: CharacterAnimator = serde_json::from_value(saved).unwrap();
    assert_eq!(loaded.rules.len(), 3);
    assert!(loaded.playing().is_none(), "a one-shot is unsaved");
}

#[test]
fn an_aim_set_picks_by_elevation_and_flips_left() {
    use tungsten_core::components::SpriteFlip;
    let dt = 1.0 / 60.0;
    let (mut world, mut schedule) = rig();
    let poses = Clips::aim(["aim_down", "aim_fwd", "aim_up"]);
    // Below its minimum speed the caster idles, its velocity its
    // direction: no Aim, no Facing.
    let caster = world.spawn_with((
        CharacterAnimator::new(poses.clone()),
        Velocity(Vec2::new(1.0, 0.0)),
        Sprite::new(SpriteAssetId::new(99)),
    ));
    let turn = |world: &mut World, velocity: Vec2| {
        world.get_mut::<Velocity>(caster).unwrap().0 = velocity;
    };
    for (velocity, want, flipped) in [
        (Vec2::new(1.0, 0.0), "aim_fwd", false),
        (Vec2::new(1.0, -2.0), "aim_up", false),
        (Vec2::new(-1.0, -2.0), "aim_up", true),
        (Vec2::new(-2.0, 1.0), "aim_fwd", true),
        (Vec2::new(-1.0, 2.0), "aim_down", true),
        // Straight down keeps the last side.
        (Vec2::new(0.0, 1.0), "aim_down", true),
        (Vec2::new(2.0, 1.0), "aim_fwd", false),
        (Vec2::new(0.0, -1.0), "aim_up", false),
    ] {
        turn(&mut world, velocity);
        frame(&mut world, &mut schedule, dt);
        assert_eq!(shown(&world, caster).1, want, "{velocity}");
        assert_eq!(
            world.get::<SpriteFlip>(caster).is_some_and(|flip| flip.0),
            flipped,
            "{velocity}"
        );
    }
    // An Aim's direction wins over the velocity's, as a one-shot's pose.
    let aim: crate::Aim =
        serde_json::from_value(json!({ "from": "intent", "direction": [-1.0, 0.0] })).unwrap();
    world.insert(caster, aim);
    turn(&mut world, Vec2::new(0.0, 1.0));
    world
        .get_mut::<CharacterAnimator>(caster)
        .unwrap()
        .play(poses);
    frame(&mut world, &mut schedule, dt);
    assert_eq!(shown(&world, caster).1, "aim_fwd");
    assert_eq!(world.get::<SpriteFlip>(caster), Some(&SpriteFlip(true)));
    // Five poses: down, down-forward, forward, up-forward, up.
    let five = Clips::aim(["aim_down", "land", "aim_fwd", "alert", "aim_up"]);
    let mut left = false;
    for (direction, want) in [
        (Vec2::new(1.0, 0.0), "aim_fwd"),
        (Vec2::new(1.0, -1.0), "alert"),
        (Vec2::new(1.0, 1.0), "land"),
        (Vec2::new(0.2, -1.0), "aim_up"),
    ] {
        assert_eq!(
            five.pick(Vec2::X, direction, &mut left).0,
            want,
            "{direction}"
        );
    }
    // The data form and its load check.
    assert_eq!(
        serde_json::to_value(&five).unwrap(),
        json!({ "aim": ["aim_down", "land", "aim_fwd", "alert", "aim_up"] })
    );
    for refused in [
        json!({ "aim": ["aim_fwd"] }),
        json!({ "aim": ["aim_down", "aim_up"], "right": "aim_fwd" }),
        json!({ "aim": [] }),
    ] {
        assert!(
            serde_json::from_value::<Clips>(refused.clone()).is_err(),
            "{refused}"
        );
    }
}

#[test]
fn an_animator_holds_its_clip_while_its_hold_passes() {
    use crate::{TagFilter, Tags, TagsPlugin};
    let (mut world, mut schedule) = rig();
    let dt = 0.04;
    let ghoul = world.spawn_with((
        walker().with_hold(TagFilter::new().all(["state.held"])),
        Tags::new(Vec::<String>::new()),
        Velocity(Vec2::ZERO),
        Sprite::new(SpriteAssetId::new(99)),
    ));
    let held = |world: &mut World, on: bool| {
        let tags = world.get_mut::<Tags>(ghoul).unwrap();
        if on {
            tags.insert("state.held");
        } else {
            tags.remove("state.held");
        }
    };
    let state = |world: &World| {
        let state = world.get::<AnimationState>(ghoul).unwrap();
        (
            state.animation_id.clone(),
            state.accumulated_ms.round() as u32,
            state.playing,
        )
    };
    frame(&mut world, &mut schedule, dt);
    frame(&mut world, &mut schedule, dt);
    assert_eq!(state(&world), ("idle".into(), 40, true));
    // Held: the tick that ran before the pass is the last.
    held(&mut world, true);
    frame(&mut world, &mut schedule, dt);
    assert_eq!(state(&world), ("idle".into(), 80, false));
    for _ in 0..5 {
        frame(&mut world, &mut schedule, dt);
    }
    assert_eq!(state(&world), ("idle".into(), 80, false), "the frame held");
    // A clip picked meanwhile starts held, on its first frame.
    world.get_mut::<Velocity>(ghoul).unwrap().0 = Vec2::new(40.0, 0.0);
    frame(&mut world, &mut schedule, dt);
    frame(&mut world, &mut schedule, dt);
    assert_eq!(state(&world), ("walk_right".into(), 0, false));
    assert_eq!(shown(&world, ghoul).2, SpriteAssetId::new(20));
    // Let go, it plays from the next tick.
    held(&mut world, false);
    frame(&mut world, &mut schedule, dt);
    assert_eq!(state(&world), ("walk_right".into(), 0, true));
    frame(&mut world, &mut schedule, dt);
    assert_eq!(state(&world), ("walk_right".into(), 40, true));
    // A one-shot's count holds with it: 300 ms of `land` outlast a held
    // second, then end 300 ms after the hold lets go.
    world.get_mut::<Velocity>(ghoul).unwrap().0 = Vec2::ZERO;
    held(&mut world, true);
    world
        .get_mut::<CharacterAnimator>(ghoul)
        .unwrap()
        .play("land");
    for _ in 0..25 {
        frame(&mut world, &mut schedule, dt);
    }
    assert_eq!(state(&world), ("land".into(), 0, false));
    held(&mut world, false);
    for _ in 0..7 {
        frame(&mut world, &mut schedule, dt);
    }
    assert_eq!(state(&world).0, "land", "240 ms of its 300 shown");
    frame(&mut world, &mut schedule, dt);
    frame(&mut world, &mut schedule, dt);
    assert_eq!(state(&world).0, "idle", "ended, the state's clip back");

    // Without a hold the animator leaves `playing` to the game.
    let idler = world.spawn_with((
        CharacterAnimator::new("idle"),
        Sprite::new(SpriteAssetId::new(99)),
    ));
    frame(&mut world, &mut schedule, dt);
    world.get_mut::<AnimationState>(idler).unwrap().playing = false;
    frame(&mut world, &mut schedule, dt);
    assert!(!world.get::<AnimationState>(idler).unwrap().playing);

    // The load checks the hold's tags; a save keeps it.
    TagsPlugin::new()
        .with_tags(["state.held"])
        .build(&mut schedule, &mut world);
    let components = world.resource::<ComponentRegistry>();
    components
        .check_in(
            "character_animator",
            &json!({ "idle": "idle", "hold": { "all": ["state.held"] } }),
            &world,
        )
        .unwrap();
    let error = components
        .check_in(
            "character_animator",
            &json!({ "idle": "idle", "hold": { "all": ["state.dazed"] } }),
            &world,
        )
        .unwrap_err();
    assert!(error.to_string().contains("hold.all.0"), "{error}");
    let saved = serde_json::to_value(world.get::<CharacterAnimator>(ghoul).unwrap()).unwrap();
    assert_eq!(saved["hold"], json!({ "all": ["state.held"] }));
    let loaded: CharacterAnimator = serde_json::from_value(saved).unwrap();
    assert_eq!(&loaded, world.get::<CharacterAnimator>(ghoul).unwrap());
    let plain = serde_json::to_value(CharacterAnimator::new("idle")).unwrap();
    assert!(plain.get("hold").is_none(), "none written by default");
}
