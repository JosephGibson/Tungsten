//! The kit's harness tests: each item through the engine's frame on
//! `Harness`, with `DefaultPlugins`, then `KitPlugins::set()` and the
//! plugins that take a game type added after them.

use std::path::PathBuf;

use glam::Vec2;
use tungsten_core::assets::{AnimationFrame, FilterMode, TextureHandle, UvRect};
use tungsten_core::physics::{
    Collider, CollisionEvent, Position, RigidBody, RigidBodyBundle, Shape, SpatialFilter, Velocity,
    overlap,
};
use tungsten_core::{
    AnimationData, AnimationRegistry, AnimationState, AssetRegistry, AudioCommand, AudioHandle,
    Bundle, CameraState, CommandBuffer, Config, Easing, Entity, EventQueue, LayerKind,
    MaterialAssetId, MaterialRegistry, MaterialUniformDefaults, PhysicsConfig, SoundData,
    SoundRegistry, Sprite, SpriteAssetId, Stage, TilemapData, TilemapInstance, TilemapRegistry,
    Time, Transform, Tween, TweenChannel, UniformOverrideBlock, Visibility, WindowSize, World,
    system,
};

use crate::App;
use crate::kit::{
    Children, Cooldown, CooldownElapsed, Counters, CueAction, CueEvent, Cues, CuesOnEvent, Damaged,
    DespawnOutside, HEALTH_TICK, HIT_FLASH_MATERIAL, Health, Hit as Outcome, HitFlash, KitPlugins,
    KitRng, KitTrace, LabelStyle, Lifetime, LocalOffset, Modifier, Parent, PathFollower,
    PlatformerController, SoundOnEvent, Source, SpawnChildren, Spawner, StateMachine,
    StateMachinePlugin, Stats, TopDownMover, TraceKind, TriggerEntered, TriggerZone,
    cursor_to_world, damage, despawn_recursive, spawn_child,
};
use crate::state::{GameState, SceneEntity, StateContext, StateId, StateStack};
use crate::testing::Harness;

/// `post_update` with `DefaultPlugins`, then `KitPlugins::set()`, the sound
/// plugin, the trigger zone and the state machine: the present phase after
/// the late propagation and before the camera.
const KIT_POST_UPDATE: &str = "post_update: physics_sync, transform_propagate, particle_count_refresh, particle_emit, particle_tick, tween_tick, squash_stretch_trigger, squash_stretch_tick, shake_tick, transform_propagate_late, kit_present, animation_tick, hit_flash_tick, camera_target, label_tick, oscillator_present, facing, character_animator, depth_sort, kit_debug_draw, hit_sound, camera_update";

/// `fixed_update` with the same plugins: the six fixed phases around
/// `physics_step`, each member in its registration order.
const KIT_FIXED_UPDATE: &str = "fixed_update: physics_prev_snapshot, kit_tick, lifetime_tick, health_tick, stats_apply, cooldown_tick, kit_intent, action_input, aim, phases, kit_move, path_follow, linear_motion, oscillator, platformer_controller, top_down_mover, steering, follow, orbit, knockback, speed_limit, physics_step, kit_overlap, despawn_outside, zone_tick, speed_limit_settle, wrap, stay_in_view, zone_entered, kit_resolve, kit_react, spawner_tick, spawn_children, cues, kit_trace_step";

/// The event the sound plugin plays.
struct Hit;

/// Hits for the test systems to send: one in `update`, one in `post_update`
/// after the sound plugin's system.
#[derive(Default)]
struct PendingHits {
    early: bool,
    late: bool,
}

/// The zone and target types of the kit app's trigger zone.
struct Zone;
struct Runner;

/// The kit app's state-machine states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Rest,
    Dash,
}

/// What the phase hooks ran, in order.
#[derive(Default)]
struct PhaseLog(Vec<&'static str>);

fn phase_log(world: &mut World, line: &'static str) {
    if let Some(log) = world.get_resource_mut::<PhaseLog>() {
        log.0.push(line);
    }
}

fn exit_rest(world: &mut World, _: Entity) {
    phase_log(world, "exit rest");
}

fn enter_dash(world: &mut World, _: Entity) {
    phase_log(world, "enter dash");
}

/// Dashes right at 300 px/s.
fn update_dash(world: &mut World, entity: Entity) {
    phase_log(world, "update dash");
    if let Some(velocity) = world.get_mut::<Velocity>(entity) {
        velocity.0 = Vec2::new(300.0, 0.0);
    }
}

fn kit_app() -> App {
    let mut app = App::new(Config::default()).unwrap();
    app.add_plugins(KitPlugins::set());
    app.add_plugin(SoundOnEvent::<Hit>::new("hit_sound", "thud"));
    app.add_plugin(TriggerZone::<Zone, Runner>::new("zone_entered"));
    app.add_plugin(
        StateMachinePlugin::<Phase>::new("phases")
            .on_exit(Phase::Rest, exit_rest)
            .on_enter(Phase::Dash, enter_dash)
            .on_update(Phase::Dash, update_dash),
    );
    app
}

fn position(harness: &Harness, entity: Entity) -> Vec2 {
    harness.world().get::<Transform>(entity).unwrap().position
}

fn spawn_at(harness: &mut Harness, x: f32, y: f32, rest: impl Bundle) -> Entity {
    harness
        .world_mut()
        .spawn_with((Transform::from_position(Vec2::new(x, y)),).with(rest))
}

fn close(a: Vec2, b: Vec2) -> bool {
    (a - b).abs().max_element() <= 1e-4
}

#[test]
fn kit_plugins_resolve_into_their_phases() {
    let mut app = kit_app();
    app.resolve_schedule().unwrap();
    let text = app.schedule().resolved_text();
    assert_eq!(text.lines().nth(4), Some(KIT_POST_UPDATE));
    assert_eq!(text.lines().nth(2), Some(KIT_FIXED_UPDATE));
    // `update` is `DefaultPlugins`' own.
    assert_eq!(text.lines().nth(3), Some("update: -"));
}

#[test]
fn animated_sprite_shows_the_current_frame_and_holds_while_paused() {
    let mut harness = Harness::new(kit_app());
    harness.set_dt(0.05);
    let (open, shut) = (SpriteAssetId::new(0), SpriteAssetId::new(1));
    let mut clips = AnimationRegistry::new();
    clips.insert(
        "blink".to_string(),
        AnimationData {
            looping: true,
            frames: vec![
                AnimationFrame {
                    sprite: open,
                    duration_ms: 100,
                },
                AnimationFrame {
                    sprite: shut,
                    duration_ms: 100,
                },
            ],
        },
    );
    harness.world_mut().insert_resource(clips);
    let eye = spawn_at(
        &mut harness,
        0.0,
        0.0,
        (AnimationState::new("blink"), Sprite::new(shut)),
    );
    let shown = |harness: &Harness| harness.world().get::<Sprite>(eye).unwrap().asset_id;

    harness.step(1);
    assert_eq!(shown(&harness), open, "frame 0 shows on the first frame");
    harness.step(1);
    assert_eq!(shown(&harness), shut, "the next after its 0.1 s");

    harness.world_mut().resource_mut::<Time>().pause();
    harness.step(10);
    assert_eq!(shown(&harness), shut, "a paused clock holds the frame");
    assert_eq!(
        harness
            .world()
            .get::<AnimationState>(eye)
            .unwrap()
            .frame_index,
        1
    );

    harness.world_mut().resource_mut::<Time>().resume();
    harness.step(2);
    assert_eq!(shown(&harness), open);
}

#[test]
fn lifetime_despawns_the_entity_and_its_children_at_its_end() {
    let mut harness = Harness::new(kit_app());
    // Two fixed steps a frame, the default bound: the lifetime counts them.
    harness.set_dt(1.0 / 30.0);
    let bolt = spawn_at(&mut harness, 0.0, 0.0, (Lifetime::new(0.12),));
    let trail = spawn_child(
        harness.world_mut(),
        bolt,
        Vec2::new(-4.0, 0.0),
        (Sprite::new(SpriteAssetId::new(0)),),
    );

    harness.step(3);
    assert!(
        harness.world().is_alive(bolt) && harness.world().is_alive(trail),
        "0.1 s of six steps"
    );

    harness.world_mut().resource_mut::<Time>().pause();
    harness.step(10);
    assert!(
        harness.world().is_alive(bolt),
        "a paused clock runs no step"
    );

    harness.world_mut().resource_mut::<Time>().resume();
    harness.step(1);
    assert!(
        !harness.world().is_alive(bolt),
        "the eighth step passed 0.12 s"
    );
    assert!(!harness.world().is_alive(trail));
}

#[test]
fn despawn_outside_takes_what_leaves_the_region_or_the_view() {
    let mut harness = Harness::new(kit_app());
    harness.world_mut().insert_resource(WindowSize {
        width: 640,
        height: 360,
    });
    let arena = DespawnOutside::region(Vec2::ZERO, Vec2::new(200.0, 100.0));
    let view = DespawnOutside::view(16.0);
    let inside = spawn_at(&mut harness, 100.0, 50.0, (arena,));
    let left_arena = spawn_at(&mut harness, 250.0, 50.0, (arena,));
    let in_view = spawn_at(&mut harness, 600.0, 300.0, (view,));
    let in_margin = spawn_at(&mut harness, 650.0, 300.0, (view,));
    let off_screen = spawn_at(&mut harness, 660.0, 300.0, (view,));
    harness.step(1);
    let world = harness.world();
    assert!(world.is_alive(inside));
    assert!(!world.is_alive(left_arena));
    assert!(world.is_alive(in_view));
    assert!(world.is_alive(in_margin), "within the 16-unit margin");
    assert!(!world.is_alive(off_screen));

    // Without `WindowSize`, the view form keeps its entities.
    harness.world_mut().remove_resource::<WindowSize>();
    let far = spawn_at(&mut harness, 1e6, 1e6, (view,));
    harness.step(1);
    assert!(harness.world().is_alive(far));
}

#[test]
fn sound_on_event_plays_each_event_once_from_any_stage() {
    let mut app = kit_app();
    app.add_system_named("send_early", |world: &mut World| {
        if std::mem::take(&mut world.resource_mut::<PendingHits>().early) {
            world.resource_mut::<EventQueue<Hit>>().send(Hit);
        }
    });
    app.add_system_to(
        Stage::PostUpdate,
        system("send_late", |world: &mut World| {
            if std::mem::take(&mut world.resource_mut::<PendingHits>().late) {
                world.resource_mut::<EventQueue<Hit>>().send(Hit);
            }
        })
        .after("hit_sound"),
    );
    let mut harness = Harness::new(app);
    let thud = harness
        .world_mut()
        .resource_mut::<SoundRegistry>()
        .register(
            "thud".to_string(),
            SoundData {
                samples: vec![0.0; 4],
                sample_rate: 44_100,
                channels: 1,
            },
            0.5,
            false,
        );
    harness.world_mut().insert_resource(PendingHits::default());
    let played = |harness: &Harness| -> Vec<(AudioHandle, f32, bool)> {
        harness
            .audio()
            .iter()
            .map(|command| match command {
                AudioCommand::Play {
                    handle,
                    volume,
                    looping,
                } => (*handle, *volume, *looping),
                other => panic!("unexpected {other:?}"),
            })
            .collect()
    };

    // An event sent in `update` plays in its own frame, once.
    harness.world_mut().resource_mut::<PendingHits>().early = true;
    harness.step(1);
    assert_eq!(played(&harness), [(thud, 0.5, false)]);
    harness.step(1);
    assert!(played(&harness).is_empty());

    // One sent after the reader in `post_update` plays the next frame, once.
    harness.world_mut().resource_mut::<PendingHits>().late = true;
    harness.step(1);
    assert!(played(&harness).is_empty());
    harness.step(1);
    assert_eq!(played(&harness), [(thud, 0.5, false)]);
    harness.step(1);
    assert!(played(&harness).is_empty());

    // Audio runs on real time: a paused game clock still plays.
    harness.world_mut().resource_mut::<Time>().pause();
    harness.world_mut().resource_mut::<PendingHits>().early = true;
    harness.step(1);
    assert_eq!(played(&harness), [(thud, 0.5, false)]);
}

#[test]
fn cursor_to_world_maps_the_harness_cursor_through_the_camera() {
    let mut harness = Harness::new(kit_app());
    harness.step(1);
    assert_eq!(cursor_to_world(harness.world()), None, "no cursor yet");
    {
        let camera = harness.world_mut().resource_mut::<CameraState>();
        camera.position = Vec2::new(100.0, 50.0);
        camera.zoom = 2.0;
    }
    harness.set_cursor(60.0, 40.0);
    harness.step(1);
    assert_eq!(
        cursor_to_world(harness.world()),
        Some(Vec2::new(130.0, 70.0))
    );
}

#[test]
fn children_follow_an_interpolated_body_at_their_offsets() {
    let mut harness = Harness::new(kit_app());
    // Frames shorter than the 1/60 s step, so the sync interpolates.
    harness.set_dt(1.0 / 90.0);
    let body = harness.world_mut().spawn_with(
        RigidBodyBundle::dynamic(Position::new(0.0, 0.0), Collider::circle(4.0))
            .with_velocity(Velocity::new(120.0, 60.0))
            .with((Transform::default(),)),
    );
    let child = spawn_child(
        harness.world_mut(),
        body,
        Vec2::new(10.0, 0.0),
        (Sprite::new(SpriteAssetId::new(0)),),
    );
    let grandchild = spawn_child(
        harness.world_mut(),
        child,
        Vec2::new(0.0, 5.0),
        (Sprite::new(SpriteAssetId::new(0)),),
    );
    let mut interpolated = 0;
    for _ in 0..12 {
        harness.step(1);
        let drawn = position(&harness, body);
        if drawn != harness.world().get::<Position>(body).unwrap().0 {
            interpolated += 1;
        }
        assert!(close(
            position(&harness, child),
            drawn + Vec2::new(10.0, 0.0)
        ));
        assert!(close(
            position(&harness, grandchild),
            drawn + Vec2::new(10.0, 5.0)
        ));
    }
    assert!(position(&harness, body).x > 0.0, "the body moved");
    assert!(interpolated > 0, "some frames drew between steps");
}

#[test]
fn children_follow_a_tweened_parent_in_the_frame_it_moves() {
    let mut harness = Harness::new(kit_app());
    let banner = spawn_at(
        &mut harness,
        0.0,
        0.0,
        (
            Tween::new(0.5, Easing::Linear).with_channel(TweenChannel::PositionX {
                from: 0.0,
                to: 100.0,
            }),
        ),
    );
    let flag = spawn_child(
        harness.world_mut(),
        banner,
        Vec2::new(0.0, -10.0),
        (Sprite::new(SpriteAssetId::new(0)),),
    );
    for _ in 0..10 {
        harness.step(1);
        let parent = position(&harness, banner);
        assert!(close(
            position(&harness, flag),
            parent + Vec2::new(0.0, -10.0)
        ));
    }
    assert!(
        position(&harness, banner).x > 0.0,
        "the tween moved the parent"
    );
}

#[test]
fn despawn_recursive_takes_the_subtree_at_the_flush() {
    let mut harness = Harness::new(kit_app());
    let ship = spawn_at(
        &mut harness,
        0.0,
        0.0,
        (Sprite::new(SpriteAssetId::new(0)),),
    );
    let turret = spawn_child(
        harness.world_mut(),
        ship,
        Vec2::new(4.0, 0.0),
        (Sprite::new(SpriteAssetId::new(0)),),
    );
    let barrel = spawn_child(
        harness.world_mut(),
        turret,
        Vec2::new(2.0, 0.0),
        (Sprite::new(SpriteAssetId::new(0)),),
    );
    let pod = spawn_child(
        harness.world_mut(),
        ship,
        Vec2::new(-4.0, 0.0),
        (Sprite::new(SpriteAssetId::new(0)),),
    );
    let sibling = spawn_at(
        &mut harness,
        50.0,
        0.0,
        (Sprite::new(SpriteAssetId::new(0)),),
    );
    {
        let commands = harness.world_mut().resource_mut::<CommandBuffer>();
        commands.remove_component::<Parent>(pod);
        despawn_recursive(commands, ship);
    }
    harness.step(1);
    let world = harness.world();
    assert!(!world.is_alive(ship));
    assert!(!world.is_alive(turret));
    assert!(!world.is_alive(barrel));
    assert!(world.is_alive(sibling));
    assert!(world.is_alive(pod), "detached earlier in the same flush");
    assert!(!world.has::<Parent>(pod));
}

#[test]
fn children_of_a_despawned_parent_go_at_the_next_propagation() {
    let mut app = kit_app();
    // `update` despawns the entity named in `DespawnNow`, directly or through
    // the command buffer.
    app.add_system_named("despawn_parents", |world: &mut World| {
        let Some(DespawnNow { entity, queued }) = world.remove_resource::<DespawnNow>() else {
            return;
        };
        if queued {
            world.resource_mut::<CommandBuffer>().despawn(entity);
        } else {
            world.despawn(entity);
        }
    });
    let mut harness = Harness::new(app);
    let direct = spawn_at(
        &mut harness,
        0.0,
        0.0,
        (Sprite::new(SpriteAssetId::new(0)),),
    );
    let direct_child = spawn_child(
        harness.world_mut(),
        direct,
        Vec2::ONE,
        (Sprite::new(SpriteAssetId::new(0)),),
    );
    let queued = spawn_at(
        &mut harness,
        0.0,
        0.0,
        (Sprite::new(SpriteAssetId::new(0)),),
    );
    let queued_child = spawn_child(
        harness.world_mut(),
        queued,
        Vec2::ONE,
        (Sprite::new(SpriteAssetId::new(0)),),
    );

    harness.world_mut().insert_resource(DespawnNow {
        entity: direct,
        queued: false,
    });
    harness.step(1);
    assert!(
        !harness.world().is_alive(direct_child),
        "a direct despawn takes them that frame"
    );

    harness.world_mut().insert_resource(DespawnNow {
        entity: queued,
        queued: true,
    });
    harness.step(1);
    assert!(!harness.world().is_alive(queued));
    assert!(
        harness.world().is_alive(queued_child),
        "a queued despawn leaves them a frame"
    );
    harness.step(1);
    assert!(!harness.world().is_alive(queued_child));
}

/// The entity `despawn_parents` takes this frame.
struct DespawnNow {
    entity: Entity,
    queued: bool,
}

#[test]
#[should_panic(expected = "holds a RigidBody")]
fn spawn_child_refuses_a_body() {
    let mut harness = Harness::new(kit_app());
    let ship = spawn_at(
        &mut harness,
        0.0,
        0.0,
        (Sprite::new(SpriteAssetId::new(0)),),
    );
    spawn_child(
        harness.world_mut(),
        ship,
        Vec2::ZERO,
        RigidBodyBundle::dynamic(Position::new(0.0, 0.0), Collider::circle(1.0)),
    );
}

#[test]
#[should_panic(expected = "has a Parent and a RigidBody")]
fn the_propagation_refuses_a_body_on_a_child() {
    let mut harness = Harness::new(kit_app());
    let ship = spawn_at(
        &mut harness,
        0.0,
        0.0,
        (Sprite::new(SpriteAssetId::new(0)),),
    );
    let pod = spawn_child(
        harness.world_mut(),
        ship,
        Vec2::ZERO,
        (Sprite::new(SpriteAssetId::new(0)),),
    );
    harness.step(1);
    harness.world_mut().insert(pod, RigidBody::dynamic());
    harness.step(1);
}

#[test]
fn the_kit_writes_only_what_the_default_extract_reads() {
    let mut harness = Harness::new(kit_app());
    let ship = spawn_at(
        &mut harness,
        10.0,
        20.0,
        (Sprite::new(SpriteAssetId::new(0)),),
    );
    let pod = spawn_child(
        harness.world_mut(),
        ship,
        Vec2::new(5.0, 0.0),
        (Sprite::new(SpriteAssetId::new(0)),),
    );
    harness.step(1);
    assert_eq!(harness.world().get::<Parent>(pod).unwrap().entity(), ship);
    assert_eq!(
        harness
            .world()
            .get::<Children>(ship)
            .unwrap()
            .iter()
            .collect::<Vec<_>>(),
        [pod]
    );
    assert_eq!(
        harness.world().get::<LocalOffset>(pod),
        Some(&LocalOffset(Vec2::new(5.0, 0.0)))
    );
    assert_eq!(position(&harness, pod), Vec2::new(15.0, 20.0));
}

#[test]
fn a_spawner_spawns_its_prefab_on_the_game_clock_at_its_position() {
    use tungsten_core::{PrefabData, PrefabRegistry, Tag};

    let dir = std::env::temp_dir().join(format!("tungsten_kit_spawner_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("slime.json");
    std::fs::write(&path, r#"{ "tag": "slime" }"#).unwrap();
    let mut harness = Harness::new(kit_app());
    let slime = PrefabData::load(&path, harness.world()).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    harness
        .world_mut()
        .resource_mut::<PrefabRegistry>()
        .insert("slime".to_string(), slime);
    // Two fixed steps a frame, the default bound: 0.1 s is three frames.
    harness.set_dt(1.0 / 30.0);
    let nest = spawn_at(
        &mut harness,
        40.0,
        24.0,
        (Spawner::new("slime", 0.1).with_count(3),),
    );
    let slimes = |harness: &Harness| {
        harness
            .world()
            .query::<(&Tag, &Transform)>()
            .filter(|(tag, _)| tag.name == "slime")
            .map(|(_, transform)| transform.position)
            .collect::<Vec<_>>()
    };

    harness.step(2);
    assert!(slimes(&harness).is_empty(), "the first after one interval");
    harness.step(1);
    assert_eq!(
        slimes(&harness),
        [Vec2::new(40.0, 24.0)],
        "spawned by the flush"
    );

    harness.world_mut().resource_mut::<Time>().pause();
    harness.step(10);
    assert_eq!(
        slimes(&harness).len(),
        1,
        "a paused clock holds the spawner"
    );

    harness.world_mut().resource_mut::<Time>().resume();
    harness.step(6);
    assert_eq!(slimes(&harness).len(), 3);
    harness.step(10);
    assert_eq!(slimes(&harness).len(), 3, "the count is spent");
    assert_eq!(
        harness.world().get::<Spawner>(nest).unwrap().remaining(),
        Some(0)
    );
}

/// A level whose `on_enter` spawns a `slime` spawner at x 10 with its
/// `SceneEntity`.
struct Level;

impl GameState for Level {
    fn id(&self) -> StateId {
        "level"
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        ctx.world.spawn_with((
            Transform::from_position(Vec2::new(10.0, 0.0)),
            Spawner::new("slime", 0.1),
            SceneEntity {
                state_id: ctx.state_id,
            },
        ));
    }

    fn on_exit(&mut self, _ctx: &mut StateContext) {}

    fn update(&mut self, _world: &mut World) {}
}

/// The kit app with prefab `slime` at dt 1/30 s (two fixed steps a frame)
/// after one frame, a control spawner at x 90 outside any state with the
/// level's interval, and `Level` pushed: both spawners first tick in the
/// next frame, so they share their phase. Returns the harness and the
/// entity count before the push.
fn level_harness(test: &str) -> (Harness, u32) {
    use tungsten_core::{PrefabData, PrefabRegistry};

    let dir = std::env::temp_dir().join(format!("tungsten_kit_{test}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("slime.json");
    std::fs::write(&path, r#"{ "tag": "slime" }"#).unwrap();
    let mut harness = Harness::new(kit_app());
    let slime = PrefabData::load(&path, harness.world()).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    harness
        .world_mut()
        .resource_mut::<PrefabRegistry>()
        .insert("slime".to_string(), slime);
    harness.set_dt(1.0 / 30.0);
    harness.step(1);
    spawn_at(&mut harness, 90.0, 0.0, (Spawner::new("slime", 0.1),));
    let count = harness.world().entity_count();
    harness
        .world_mut()
        .resource_mut::<StateStack>()
        .request_push(Level);
    (harness, count)
}

/// Each slime's x, its spawner's (10 the level's, 90 the control's), and
/// its state marker's state, by x.
fn slimes_and_states(harness: &Harness) -> Vec<(f32, Option<StateId>)> {
    use tungsten_core::Tag;

    let mut found: Vec<_> = harness
        .world()
        .query::<(&Tag, &Transform, Option<&SceneEntity>)>()
        .filter(|(tag, ..)| tag.name == "slime")
        .map(|(_, transform, marker)| (transform.position.x, marker.map(|m| m.state_id)))
        .collect();
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    found
}

fn pop_state(harness: &mut Harness) {
    harness
        .world_mut()
        .resource_mut::<StateStack>()
        .request_pop();
}

#[test]
fn a_spawners_entities_leave_with_its_state() {
    let (mut harness, count) = level_harness("leave_with_state");
    // Twelve steps: two periods.
    harness.step(6);
    assert_eq!(
        slimes_and_states(&harness),
        [
            (10.0, Some("level")),
            (10.0, Some("level")),
            (90.0, None),
            (90.0, None),
        ],
        "the level's spawns take its marker, the control's none"
    );
    pop_state(&mut harness);
    harness.step(1);
    assert_eq!(
        slimes_and_states(&harness),
        [(90.0, None), (90.0, None)],
        "nothing of the level remains"
    );
    assert_eq!(harness.world().query::<&SceneEntity>().count(), 0);
    assert_eq!(harness.world().entity_count(), count + 2);
}

#[test]
fn a_spawner_due_in_its_states_exit_frame_leaves_nothing() {
    let (mut harness, count) = level_harness("exit_frame");
    harness.step(3);
    assert_eq!(
        slimes_and_states(&harness),
        [(10.0, Some("level")), (90.0, None)]
    );
    // The pop's frame holds the twelfth step, the next period's end for
    // both spawners.
    harness.step(2);
    pop_state(&mut harness);
    harness.step(1);
    assert_eq!(
        slimes_and_states(&harness),
        [(90.0, None), (90.0, None)],
        "the control spawned in the exit frame; the level left no spawn"
    );
    assert_eq!(
        harness.world().query::<&Spawner>().count(),
        1,
        "the control"
    );
    assert_eq!(
        harness.world().entity_count(),
        count + 2,
        "the control and its two spawns"
    );
}

#[test]
fn a_child_spawner_on_a_moving_parent_spawns_at_its_last_propagated_point() {
    use tungsten_core::{PrefabData, PrefabRegistry, Tag};

    let dir = std::env::temp_dir().join(format!("tungsten_kit_child_nest_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("slime.json");
    std::fs::write(&path, r#"{ "tag": "slime" }"#).unwrap();
    let mut harness = Harness::new(kit_app());
    let slime = PrefabData::load(&path, harness.world()).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    harness
        .world_mut()
        .resource_mut::<PrefabRegistry>()
        .insert("slime".to_string(), slime);
    // A cart moving right one pixel a step, with a nest hanging off it that
    // spawns once a step.
    let cart = harness.world_mut().spawn_with(
        RigidBodyBundle::kinematic(Position::new(0.0, 0.0), Collider::aabb(Vec2::splat(4.0)))
            .with_velocity(Velocity::new(60.0, 0.0))
            .with((Transform::default(),)),
    );
    let nest = spawn_child(
        harness.world_mut(),
        cart,
        Vec2::new(10.0, 0.0),
        (Spawner::new("slime", STEP * 0.99),),
    );
    let slimes = |harness: &Harness| -> Vec<f32> {
        let mut found: Vec<f32> = harness
            .world()
            .query::<(&Tag, &Transform)>()
            .filter(|(tag, _)| tag.name == "slime")
            .map(|(_, transform)| transform.position.x)
            .collect();
        found.sort_by(f32::total_cmp);
        found
    };
    harness.step(1);
    for _ in 0..3 {
        let nest_drawn = position(&harness, nest).x;
        let cart_before = body_at(&harness, cart).x;
        let count = slimes(&harness).len();
        harness.step(1);
        let now = slimes(&harness);
        assert_eq!(now.len(), count + 1, "one spawn a step");
        let newest = now[now.len() - 1];
        assert_eq!(newest, nest_drawn, "where the nest was last drawn");
        assert_eq!(body_at(&harness, cart).x, cart_before + 1.0);
        assert!(
            newest < body_at(&harness, cart).x + 10.0,
            "behind the cart's position: a recorded limit"
        );
    }
    // The spawns carry the chain's origin: the nest, which has no source.
    assert!(
        harness
            .world()
            .query::<(&Tag, &Source)>()
            .filter(|(tag, _)| tag.name == "slime")
            .all(|(_, source)| source.entity() == nest)
    );
}

// The controllers and gameplay helpers (W13c and W13d), each through the
// engine's frame: at the harness's 1/60 s dt each frame runs one fixed step.

const GRAVITY: f32 = 3600.0;
const RUN: f32 = 560.0;
const JUMP: f32 = 1280.0;
const HERO_HALF: Vec2 = Vec2::new(10.0, 20.0);
const STEP: f32 = 1.0 / 60.0;

/// The kit app with example 01's gravity.
fn gravity_harness() -> Harness {
    let mut harness = Harness::new(kit_app());
    harness.world_mut().resource_mut::<PhysicsConfig>().gravity = Vec2::new(0.0, GRAVITY);
    harness
}

/// A map of 16 px tiles at the origin, `cols` by 12 rows, solid at `solid`
/// (column, row) cells.
fn tile_map(harness: &mut Harness, cols: u32, solid: impl IntoIterator<Item = (u32, u32)>) {
    let mut map = TilemapData::new(16, 16, cols, 12);
    let tile = map.add_tile("solid");
    let layer = map.add_layer("solid", LayerKind::Collision);
    for (col, row) in solid {
        map.set_tile(layer, col, row, tile);
    }
    let world = harness.world_mut();
    world
        .resource_mut::<TilemapRegistry>()
        .insert("level".into(), map);
    let instance = world.spawn();
    world.insert(instance, TilemapInstance::new("level", Vec2::ZERO));
}

/// Row 11's columns `0..cols`: a floor whose top is at y 176.
fn floor_row(cols: u32) -> impl Iterator<Item = (u32, u32)> {
    (0..cols).map(|col| (col, 11))
}

/// A platformer whose feet are at `feet`.
fn hero(harness: &mut Harness, feet: Vec2, controller: PlatformerController) -> Entity {
    harness.world_mut().spawn_with(
        RigidBodyBundle::kinematic(
            Position(feet - Vec2::new(0.0, HERO_HALF.y)),
            Collider::aabb(HERO_HALF),
        )
        .with((controller,)),
    )
}

fn body_at(harness: &Harness, entity: Entity) -> Vec2 {
    harness.world().get::<Position>(entity).unwrap().0
}

fn pc(harness: &mut Harness, entity: Entity) -> &mut PlatformerController {
    harness
        .world_mut()
        .get_mut::<PlatformerController>(entity)
        .unwrap()
}

fn pc_ref(harness: &Harness, entity: Entity) -> &PlatformerController {
    harness.world().get::<PlatformerController>(entity).unwrap()
}

/// Nothing but `entity` overlaps its box, at the last step's positions.
fn clear_of_everything(harness: &Harness, entity: Entity, half: Vec2) -> bool {
    let mut met = Vec::new();
    overlap(
        harness.world(),
        Shape::Aabb { half_extents: half },
        body_at(harness, entity),
        &SpatialFilter::new().excluding(entity),
        &mut met,
    );
    met.is_empty()
}

/// The least y the body reaches over `frames` frames.
fn top_over(harness: &mut Harness, entity: Entity, frames: u32) -> f32 {
    let mut top = body_at(harness, entity).y;
    for _ in 0..frames {
        harness.step(1);
        top = top.min(body_at(harness, entity).y);
    }
    top
}

#[test]
fn a_platformer_in_a_fresh_world_never_enters_a_floor_wall_box_or_ceiling() {
    let mut harness = gravity_harness();
    // A floor at y 176, a tile wall whose left face is at x 624, a box whose
    // right face is at x 100, and a slab whose underside is at y 96.
    tile_map(
        &mut harness,
        40,
        floor_row(40).chain((0..11).map(|row| (39, row))),
    );
    let world = harness.world_mut();
    world.spawn_with(RigidBodyBundle::r#static(
        Position::new(90.0, 136.0),
        Collider::aabb(Vec2::new(10.0, 40.0)),
    ));
    world.spawn_with(RigidBodyBundle::r#static(
        Position::new(150.0, 90.0),
        Collider::aabb(Vec2::new(60.0, 6.0)),
    ));
    // Against the box, 4 px above the floor, running into the box from
    // before its first frame.
    let mut controller = PlatformerController::new(RUN, JUMP);
    controller.set_run(-1.0);
    let entity = hero(&mut harness, Vec2::new(110.0, 172.0), controller);
    let frames = |harness: &mut Harness, count: u32| {
        for _ in 0..count {
            harness.step(1);
            assert!(clear_of_everything(harness, entity, HERO_HALF));
        }
    };

    frames(&mut harness, 20);
    let at = body_at(&harness, entity);
    assert!(pc_ref(&harness, entity).grounded(), "it landed");
    assert!((at.x - 110.0).abs() < 1e-3, "the box held it: {at}");
    assert!(
        (at.y + HERO_HALF.y - 176.0).abs() < 0.02,
        "on the floor: {at}"
    );

    pc(&mut harness, entity).set_run(1.0);
    frames(&mut harness, 70);
    let at = body_at(&harness, entity);
    assert!(
        (at.x + HERO_HALF.x - 624.0).abs() < 0.02,
        "the tile wall: {at}"
    );

    pc(&mut harness, entity).set_run(-1.0);
    frames(&mut harness, 70);
    let at = body_at(&harness, entity);
    assert!((at.x - HERO_HALF.x - 100.0).abs() < 0.02, "the box: {at}");

    pc(&mut harness, entity).set_run(0.0);
    pc(&mut harness, entity).press_jump();
    let mut head = f32::MAX;
    for _ in 0..40 {
        harness.step(1);
        assert!(clear_of_everything(&harness, entity, HERO_HALF));
        head = head.min(body_at(&harness, entity).y - HERO_HALF.y);
    }
    assert!(
        (head - 96.0).abs() < 0.02,
        "the slab stopped its rise: {head}"
    );
    assert!(pc_ref(&harness, entity).grounded());
}

/// The height a jump from a tile floor reaches; the jump is released after
/// `held` frames when given.
fn jump_height(controller: PlatformerController, held: Option<u32>) -> f32 {
    let mut harness = gravity_harness();
    tile_map(&mut harness, 40, floor_row(40));
    let entity = hero(&mut harness, Vec2::new(300.0, 176.0), controller);
    harness.step(2);
    assert!(pc_ref(&harness, entity).grounded());
    let start = body_at(&harness, entity).y;
    pc(&mut harness, entity).press_jump();
    let mut top = start;
    if let Some(held) = held {
        top = top.min(top_over(&mut harness, entity, held));
        pc(&mut harness, entity).set_jump_held(false);
    }
    start - top.min(top_over(&mut harness, entity, 40))
}

#[test]
fn a_held_jump_peaks_at_its_height_and_an_early_release_cuts_it() {
    let exact = JUMP * JUMP / (2.0 * GRAVITY);
    let held = jump_height(PlatformerController::new(RUN, JUMP), None);
    assert!(held <= exact && exact - held < 0.25, "{held} vs {exact}");
    let released = jump_height(PlatformerController::new(RUN, JUMP), Some(5));
    assert!(released < held - 50.0, "{released}");
    let uncut = jump_height(
        PlatformerController::new(RUN, JUMP).with_jump_cut(1.0),
        Some(5),
    );
    assert!(held - uncut < 0.25 && uncut <= exact, "{uncut}");
}

/// Whether a press made `late` frames after the platformer walked off a
/// ledge jumps.
fn jumps_after_the_edge(late: u32) -> bool {
    let mut harness = gravity_harness();
    tile_map(&mut harness, 40, floor_row(20));
    let mut controller = PlatformerController::new(120.0, JUMP);
    controller.set_run(1.0);
    let entity = hero(&mut harness, Vec2::new(300.0, 176.0), controller);
    harness.step(2);
    while pc_ref(&harness, entity).grounded() {
        harness.step(1);
    }
    harness.step(late);
    pc(&mut harness, entity).press_jump();
    harness.step(1);
    pc_ref(&harness, entity).jumped()
}

/// Whether a press made `early` frames before a fall lands jumps on the
/// landing.
fn jumps_on_landing(early: u32) -> bool {
    let drop = || {
        let mut harness = gravity_harness();
        tile_map(&mut harness, 40, floor_row(40));
        let entity = hero(
            &mut harness,
            Vec2::new(300.0, 56.0),
            PlatformerController::new(RUN, JUMP),
        );
        (harness, entity)
    };
    let (mut harness, entity) = drop();
    let mut landing = 0;
    while !pc_ref(&harness, entity).grounded() {
        harness.step(1);
        landing += 1;
    }
    let (mut harness, entity) = drop();
    harness.step(landing - early);
    pc(&mut harness, entity).press_jump();
    harness.step(early);
    assert!(pc_ref(&harness, entity).grounded() || early == 0);
    harness.step(1);
    pc_ref(&harness, entity).jumped()
}

#[test]
fn coyote_time_and_the_jump_buffer_take_presses_inside_their_windows_only() {
    assert!(jumps_after_the_edge(0));
    assert!(jumps_after_the_edge(3));
    assert!(!jumps_after_the_edge(8));
    assert!(jumps_on_landing(1));
    assert!(jumps_on_landing(4));
    assert!(!jumps_on_landing(9));
}

#[test]
fn a_platformer_rides_a_ping_pong_platform_both_ways_and_leaves_it_on_a_jump() {
    let mut harness = gravity_harness();
    let deck = harness.world_mut().spawn_with(
        RigidBodyBundle::kinematic(
            Position::new(200.0, 150.0),
            Collider::aabb(Vec2::new(40.0, 8.0)),
        )
        .with((PathFollower::new(
            [Vec2::new(200.0, 150.0), Vec2::new(400.0, 150.0)],
            100.0,
        )
        .ping_pong()
        .with_easing(Easing::SineInOut),)),
    );
    let entity = hero(
        &mut harness,
        Vec2::new(200.0, 140.0),
        PlatformerController::new(RUN, JUMP),
    );
    harness.step(6);
    assert!(pc_ref(&harness, entity).grounded(), "it landed on the deck");
    let offset = body_at(&harness, entity).x - body_at(&harness, deck).x;
    let (mut right, mut left) = (f32::MIN, f32::MAX);
    for _ in 0..270 {
        harness.step(1);
        let deck_at = body_at(&harness, deck);
        let at = body_at(&harness, entity);
        assert!(pc_ref(&harness, entity).grounded());
        assert!(
            (at.x - deck_at.x - offset).abs() < 0.05,
            "{at} on {deck_at}"
        );
        assert!((deck_at.y - 8.0 - (at.y + HERO_HALF.y)).abs() < 0.02);
        right = right.max(deck_at.x);
        left = left.min(deck_at.x);
    }
    assert!(right > 399.0 && left < 201.0, "both ways: {left}..{right}");

    pc(&mut harness, entity).press_jump();
    harness.step(3);
    assert!(!pc_ref(&harness, entity).grounded());
    let x = body_at(&harness, entity).x;
    let deck_x = body_at(&harness, deck).x;
    harness.step(5);
    assert_eq!(body_at(&harness, entity).x, x, "no carry in the air");
    assert_ne!(body_at(&harness, deck).x, deck_x);
}

#[test]
fn a_launch_holds_its_speed_for_the_lock_and_a_push_each_step_bends_the_path() {
    let mut harness = gravity_harness();
    tile_map(&mut harness, 60, floor_row(60));
    let mut controller = PlatformerController::new(RUN, JUMP);
    controller.set_run(1.0);
    let entity = hero(&mut harness, Vec2::new(500.0, 176.0), controller);
    harness.step(2);
    {
        let controller = pc(&mut harness, entity);
        controller.launch(Vec2::new(-380.0, -420.0));
        controller.lock_input(0.18);
    }
    // The 0.18 s lock covers 11 steps of knockback against the run input.
    let mut x = body_at(&harness, entity).x;
    for _ in 0..11 {
        harness.step(1);
        let next = body_at(&harness, entity).x;
        assert!((next - x + 380.0 * STEP).abs() < 0.01, "{next} after {x}");
        x = next;
    }
    harness.step(1);
    let next = body_at(&harness, entity).x;
    assert!(
        (next - x - RUN * STEP).abs() < 0.01,
        "the run steers: {next}"
    );

    // A pull of half the gravity each step takes a jump about twice as high.
    let plain = jump_height(PlatformerController::new(RUN, JUMP), None);
    let mut harness = gravity_harness();
    tile_map(&mut harness, 40, floor_row(40));
    let entity = hero(
        &mut harness,
        Vec2::new(300.0, 176.0),
        PlatformerController::new(RUN, JUMP),
    );
    harness.step(2);
    let start = body_at(&harness, entity).y;
    pc(&mut harness, entity).press_jump();
    let mut top = start;
    for _ in 0..60 {
        pc(&mut harness, entity).push(Vec2::new(0.0, -GRAVITY * STEP * 0.5));
        harness.step(1);
        top = top.min(body_at(&harness, entity).y);
    }
    let pulled = start - top;
    assert!(
        pulled > 1.9 * plain && pulled < 2.1 * plain,
        "{pulled} vs {plain}"
    );
}

#[test]
fn a_top_down_mover_slides_along_a_wall_and_a_body_pressing_into_it_reports_the_contact() {
    let mut harness = Harness::new(kit_app());
    harness.world_mut().spawn_with(RigidBodyBundle::r#static(
        Position::new(300.0, 0.0),
        Collider::aabb(Vec2::new(10.0, 400.0)),
    ));
    let mut mover = TopDownMover::new(120.0);
    mover.set_direction(Vec2::new(1.0, 1.0));
    let half = Vec2::splat(10.0);
    let entity = harness.world_mut().spawn_with(
        RigidBodyBundle::kinematic(Position::new(280.0, 0.0), Collider::aabb(half)).with((mover,)),
    );
    let mut y = 0.0;
    for _ in 0..30 {
        harness.step(1);
        let at = body_at(&harness, entity);
        assert!(at.x <= 280.0 + 1e-3, "never into the wall: {at}");
        assert!(clear_of_everything(&harness, entity, half));
        assert!(at.y >= y);
        y = at.y;
    }
    assert!(y > 35.0, "it slid along: {y}");

    // A crate in its way stops it, and pressing into it reports the contact.
    let crate_at = Vec2::new(280.0, y + 50.0);
    let crate_ = harness.world_mut().spawn_with(RigidBodyBundle::dynamic(
        Position(crate_at),
        Collider::aabb(half),
    ));
    let mut pressed = false;
    for frame in 0..40 {
        if frame >= 30 {
            harness.world_mut().get_mut::<Velocity>(crate_).unwrap().0 = Vec2::new(0.0, -60.0);
        }
        harness.step(1);
        let at = body_at(&harness, entity);
        assert!(at.y + half.y <= body_at(&harness, crate_).y - half.y + 0.3);
        pressed |= harness
            .events::<CollisionEvent>()
            .any(|event| event.a == crate_ && event.b == Some(entity));
    }
    assert!(pressed, "the crate's press named it as `a`");
    assert!(
        (body_at(&harness, crate_).y - crate_at.y).abs() < 1.0,
        "the mover never pushed the crate"
    );
}

/// The steps, counted from 1, at which `striker` hit with health.
#[derive(Default)]
struct Strikes {
    step: u32,
    hits: Vec<(u32, Outcome)>,
}

/// Hits the target of `Strikes` once a fixed step for 60 steps at frames of
/// `dt`; returns the hits that landed and the `Damaged` events read.
fn strike_every_step(dt: f32) -> (Vec<(u32, Outcome)>, Vec<Damaged>) {
    let mut app = kit_app();
    app.add_system_to(
        Stage::FixedUpdate,
        system("striker", |world: &mut World| {
            let target = *world.resource::<Entity>();
            let strikes = world.resource_mut::<Strikes>();
            strikes.step += 1;
            if strikes.step > 60 {
                return;
            }
            let step = strikes.step;
            let outcome = damage(world, target, 1.0);
            if outcome != Outcome::Ignored {
                world.resource_mut::<Strikes>().hits.push((step, outcome));
            }
        })
        .after(HEALTH_TICK),
    );
    let mut harness = Harness::new(app);
    harness.set_dt(dt);
    let target = harness
        .world_mut()
        .spawn_with((Health::new(10.0).with_invulnerability(0.1),));
    harness.world_mut().insert_resource(target);
    harness.world_mut().insert_resource(Strikes::default());
    let mut sent = Vec::new();
    while harness.world().resource::<Strikes>().step < 60 {
        harness.step(1);
        sent.extend(harness.events::<Damaged>().copied());
    }
    let hits = std::mem::take(&mut harness.world_mut().resource_mut::<Strikes>().hits);
    (hits, sent)
}

#[test]
fn a_hit_sequence_on_the_fixed_step_lands_the_same_hits_at_any_frame_rate() {
    let (hits, sent) = strike_every_step(1.0 / 60.0);
    // A 0.1 s window is six steps: hits at steps 1, 7, ... 55, the tenth
    // killing.
    let expected: Vec<(u32, Outcome)> = (0..10)
        .map(|n| {
            let outcome = if n == 9 {
                Outcome::Killed
            } else {
                Outcome::Hurt
            };
            (1 + 6 * n, outcome)
        })
        .collect();
    assert_eq!(hits, expected);
    assert_eq!(sent.len(), 10, "one Damaged per hit applied");
    assert!(sent[9].killed && !sent[8].killed);
    for dt in [1.0 / 30.0, 1.0 / 144.0] {
        let (other, other_sent) = strike_every_step(dt);
        assert_eq!(other, expected, "at dt {dt}");
        assert_eq!(other_sent, sent, "at dt {dt}");
    }
}

#[test]
fn the_window_holds_while_paused() {
    let mut harness = Harness::new(kit_app());
    let target = harness
        .world_mut()
        .spawn_with((Health::new(5.0).with_invulnerability(0.1),));
    assert_eq!(damage(harness.world_mut(), target, 1.0), Outcome::Hurt);
    harness.world_mut().resource_mut::<Time>().pause();
    harness.step(30);
    assert!(
        harness
            .world()
            .get::<Health>(target)
            .unwrap()
            .is_invulnerable()
    );
    assert_eq!(damage(harness.world_mut(), target, 1.0), Outcome::Ignored);
    harness.world_mut().resource_mut::<Time>().resume();
    harness.step(6);
    assert_eq!(damage(harness.world_mut(), target, 1.0), Outcome::Hurt);
    assert_eq!(
        harness.world().get::<Health>(target).unwrap().current(),
        3.0
    );
}

/// The batch that draws the sprite at `x`.
fn batch_at(harness: &Harness, x: f32) -> (Option<MaterialAssetId>, Option<UniformOverrideBlock>) {
    let draw = harness.draw().unwrap();
    let batch = draw
        .sprites
        .iter()
        .find(|batch| batch.instances.iter().any(|sprite| sprite.position[0] == x))
        .unwrap();
    (batch.material_id, batch.uniform_overrides)
}

#[test]
fn a_flash_draws_through_damage_flash_and_restores_the_sprites_material_and_block() {
    let mut harness = Harness::new(kit_app());
    let world = harness.world_mut();
    let quad = world.resource_mut::<AssetRegistry>().register_sprite(
        "quad".into(),
        FilterMode::Nearest,
        16,
        16,
        PathBuf::from("test/quad.png"),
        TextureHandle(0),
        UvRect::FULL,
        None,
        None,
        None,
    );
    let mut materials = MaterialRegistry::new();
    let glow = materials.allocate(
        "glow",
        PathBuf::from("test/manifest.json"),
        "glow".into(),
        MaterialUniformDefaults::default(),
    );
    let flash = materials.allocate(
        HIT_FLASH_MATERIAL,
        PathBuf::from("test/manifest.json"),
        "damage_flash".into(),
        MaterialUniformDefaults::default(),
    );
    world.insert_resource(materials);
    let own = UniformOverrideBlock::from_slots(
        [[0.0; 4], [0.5, 0.5, 0.5, 1.0], [0.0; 4], [0.0; 4]],
        [0.25, 0.75, 0.0, 0.0],
        [0; 4],
    );
    let red = [1.0, 0.0, 0.0, 1.0];
    let glowing = world.spawn_with((
        Transform::from_position(Vec2::new(0.0, 0.0)),
        Sprite::new(quad).with_material(glow),
        Visibility::default(),
        own,
        Health::new(5.0),
        HitFlash::new(red, 0.1),
    ));
    let plain = world.spawn_with((
        Transform::from_position(Vec2::new(100.0, 0.0)),
        Sprite::new(quad),
        Visibility::default(),
        Health::new(5.0),
        HitFlash::new([1.0; 4], 0.1),
    ));
    harness.step(1);
    assert_eq!(batch_at(&harness, 0.0), (Some(glow), Some(own)));
    assert_eq!(batch_at(&harness, 100.0), (None, None));

    damage(harness.world_mut(), glowing, 1.0);
    damage(harness.world_mut(), plain, 1.0);
    let mut glowing_amounts = Vec::new();
    let mut plain_amounts = Vec::new();
    for _ in 0..6 {
        harness.step(1);
        let (material, block) = batch_at(&harness, 0.0);
        assert_eq!(material, Some(flash));
        let block = block.unwrap();
        assert_eq!(block.vec4[0], red);
        assert_eq!(block.vec4[1], own.vec4[1], "its own values stay");
        glowing_amounts.push(block.f32s[0]);
        let (material, block) = batch_at(&harness, 100.0);
        assert_eq!(material, Some(flash));
        plain_amounts.push(block.unwrap().f32s[0]);
    }
    assert_eq!(glowing_amounts[0], 1.0, "full strength in its first frame");
    assert!(glowing_amounts.windows(2).all(|pair| pair[1] < pair[0]));
    assert_eq!(glowing_amounts, plain_amounts);
    harness.step(1);
    assert_eq!(batch_at(&harness, 0.0), (Some(glow), Some(own)));
    assert_eq!(batch_at(&harness, 100.0), (None, None));
    assert!(!harness.world().has::<UniformOverrideBlock>(plain));
}

#[test]
fn a_trigger_zone_reports_each_entry_of_its_target_type_only() {
    let mut harness = Harness::new(kit_app());
    tile_map(&mut harness, 40, floor_row(40));
    // A zone that overlaps the floor's tiles from its first step.
    let zone = harness.world_mut().spawn_with(
        RigidBodyBundle::sensor(
            Position::new(300.0, 160.0),
            Collider::aabb(Vec2::new(30.0, 20.0)),
        )
        .with((Zone,)),
    );
    let runner = harness.world_mut().spawn_with(
        RigidBodyBundle::kinematic(
            Position::new(200.0, 150.0),
            Collider::aabb(Vec2::splat(10.0)),
        )
        .with_velocity(Velocity::new(600.0, 0.0))
        .with((Runner,)),
    );
    harness.world_mut().spawn_with(
        RigidBodyBundle::kinematic(
            Position::new(400.0, 150.0),
            Collider::aabb(Vec2::splat(10.0)),
        )
        .with_velocity(Velocity::new(-600.0, 0.0)),
    );
    let mut entered = Vec::new();
    let mut frames = |harness: &mut Harness, count: u32| {
        for _ in 0..count {
            harness.step(1);
            entered.extend(harness.events::<TriggerEntered<Zone, Runner>>().copied());
        }
    };
    frames(&mut harness, 30);
    harness.world_mut().get_mut::<Velocity>(runner).unwrap().0.x = -600.0;
    frames(&mut harness, 30);
    assert_eq!(
        entered,
        [
            TriggerEntered::new(zone, runner),
            TriggerEntered::new(zone, runner)
        ]
    );
}

/// The point `distance` px along the loop (0, 0), (60, 0), (60, 60),
/// (0, 60).
fn on_square(distance: f32) -> Vec2 {
    let d = distance.rem_euclid(240.0);
    match d {
        d if d < 60.0 => Vec2::new(d, 0.0),
        d if d < 120.0 => Vec2::new(60.0, d - 60.0),
        d if d < 180.0 => Vec2::new(180.0 - d, 60.0),
        d => Vec2::new(0.0, 240.0 - d),
    }
}

#[test]
fn looping_and_finished_paths_end_every_step_on_the_path() {
    let mut harness = Harness::new(kit_app());
    let square = [
        Vec2::new(0.0, 0.0),
        Vec2::new(60.0, 0.0),
        Vec2::new(60.0, 60.0),
        Vec2::new(0.0, 60.0),
    ];
    let body = |harness: &mut Harness, follower: PathFollower, start: Vec2| {
        harness.world_mut().spawn_with(
            RigidBodyBundle::kinematic(Position(start), Collider::aabb(Vec2::splat(4.0)))
                .with((follower,)),
        )
    };
    let looping = body(
        &mut harness,
        PathFollower::new(square, 120.0).looping(),
        Vec2::ZERO,
    );
    let end = Vec2::new(160.0, 100.0);
    let once = body(
        &mut harness,
        PathFollower::new([Vec2::new(100.0, 100.0), end], 120.0),
        Vec2::new(100.0, 100.0),
    );
    for n in 1..=300 {
        harness.step(1);
        let at = body_at(&harness, looping);
        let expected = on_square(120.0 * STEP * n as f32);
        assert!(
            (at - expected).length() < 0.01,
            "frame {n}: {at} vs {expected}"
        );
        let travelled = (120.0 * STEP * n as f32).min(60.0);
        let at = body_at(&harness, once);
        assert!(
            (at - Vec2::new(100.0 + travelled, 100.0)).length() < 0.01,
            "{at}"
        );
    }
    assert!(
        harness
            .world()
            .get::<PathFollower>(once)
            .unwrap()
            .is_finished()
    );
    assert_eq!(harness.world().get::<Velocity>(once).unwrap().0, Vec2::ZERO);
    assert!((body_at(&harness, once) - end).length() < 0.01);
}

#[test]
fn a_state_machine_transitions_before_the_step_on_the_game_clock() {
    let mut harness = Harness::new(kit_app());
    harness.world_mut().insert_resource(PhaseLog::default());
    let entity = harness.world_mut().spawn_with(
        RigidBodyBundle::kinematic(Position::new(0.0, 0.0), Collider::aabb(Vec2::splat(4.0)))
            .with((StateMachine::new(Phase::Rest),)),
    );
    harness.step(3);
    assert!(harness.world().resource::<PhaseLog>().0.is_empty());
    harness
        .world_mut()
        .get_mut::<StateMachine<Phase>>(entity)
        .unwrap()
        .go(Phase::Dash);
    harness.step(1);
    assert_eq!(
        harness.world().resource::<PhaseLog>().0,
        ["exit rest", "enter dash", "update dash"]
    );
    // The update hook's velocity moved the body in the same step.
    assert!((body_at(&harness, entity).x - 300.0 * STEP).abs() < 1e-3);
    let machine = harness.world().get::<StateMachine<Phase>>(entity).unwrap();
    assert_eq!(machine.previous(), Some(Phase::Rest));
    assert!((machine.elapsed() - STEP).abs() < 1e-6);

    // At half speed, 20 frames run 10 steps of game time.
    harness.world_mut().resource_mut::<Time>().set_scale(0.5);
    harness.step(20);
    let elapsed = harness
        .world()
        .get::<StateMachine<Phase>>(entity)
        .unwrap()
        .elapsed();
    assert!((elapsed - 11.0 * STEP).abs() < 1e-4, "{elapsed}");
}

#[test]
fn stats_drive_the_movers_health_and_cooldowns() {
    let mut harness = Harness::new(kit_app());
    harness.set_dt(1.0 / 60.0);
    let mut mover = TopDownMover::new(240.0);
    mover.set_direction(Vec2::X);
    let runner = spawn_at(
        &mut harness,
        0.0,
        0.0,
        (
            Position::new(0.0, 0.0),
            mover,
            Health::new(5.0),
            Cooldown::new(0.5),
            Stats::new()
                .with(Stats::MOVE_SPEED, 60.0)
                .with(Stats::MAX_HEALTH, 8.0)
                .with(Stats::COOLDOWN_RATE, 2.0),
        ),
    );
    let mut elapsed = 0;
    for _ in 0..60 {
        harness.step(1);
        elapsed += harness.events::<CooldownElapsed>().count();
    }
    let world = harness.world();
    let at = world.get::<Position>(runner).unwrap().0;
    assert!(
        (at.x - 60.0).abs() < 1e-3,
        "60 px/s from its first step: {at}"
    );
    let health = world.get::<Health>(runner).unwrap();
    assert_eq!(
        (health.current(), health.max()),
        (8.0, 8.0),
        "5/5 becomes 8/8"
    );
    assert_eq!(elapsed, 4, "a 0.5 s cooldown at twice the game clock");

    // A modifier doubles the speed from the next step.
    harness
        .world_mut()
        .get_mut::<Stats>(runner)
        .unwrap()
        .add_modifier(Stats::MOVE_SPEED, "haste", Modifier::Pct(1.0));
    harness.step(30);
    let at = harness.world().get::<Position>(runner).unwrap().0;
    assert!((at.x - 120.0).abs() < 1e-3, "{at}");

    // An entity without stats keeps its own fields.
    let plain = spawn_at(
        &mut harness,
        0.0,
        50.0,
        (Position::new(0.0, 50.0), Health::new(5.0)),
    );
    harness.step(1);
    assert_eq!(harness.world().get::<Health>(plain).unwrap().max(), 5.0);
}

/// The frames, at dt 1/30, on which `entity`'s cooldown elapsed, with how
/// many times each.
fn elapses(harness: &mut Harness, entity: Entity, frames: u32) -> Vec<(u32, usize)> {
    (0..frames)
        .filter_map(|frame| {
            harness.step(1);
            let count = harness
                .events::<CooldownElapsed>()
                .filter(|event| event.entity == entity)
                .count();
            (count > 0).then_some((frame, count))
        })
        .collect()
}

/// A jittered cooldown's elapses over three seconds under `seed`.
fn jittered_run(seed: u64) -> Vec<(u32, usize)> {
    let mut harness = Harness::new(kit_app());
    harness.set_dt(1.0 / 30.0);
    harness.world_mut().resource_mut::<KitRng>().reseed(seed);
    let hopper = spawn_at(
        &mut harness,
        0.0,
        0.0,
        (Cooldown::new(0.25).with_jitter(0.25),),
    );
    elapses(&mut harness, hopper, 90)
}

#[test]
fn a_cooldown_elapses_on_the_game_clock_and_its_jitter_repeats_under_a_seed() {
    // Two fixed steps a frame: a 0.25 s cooldown elapses every 7.5 frames.
    let mut harness = Harness::new(kit_app());
    harness.set_dt(1.0 / 30.0);
    let fuse = spawn_at(&mut harness, 0.0, 0.0, (Cooldown::new(0.25),));
    let seen = elapses(&mut harness, fuse, 30);
    assert_eq!(seen.iter().map(|&(_, n)| n).sum::<usize>(), 4, "{seen:?}");

    // A paused clock holds it.
    harness.world_mut().resource_mut::<Time>().pause();
    assert!(elapses(&mut harness, fuse, 30).is_empty());
    harness.world_mut().resource_mut::<Time>().resume();
    assert_eq!(elapses(&mut harness, fuse, 30).len(), 4);

    let first = jittered_run(11);
    assert_eq!(first, jittered_run(11), "one seed, one run");
    assert_ne!(first, jittered_run(12));
    assert!(
        (6..=12).contains(&first.len()),
        "0.25 to 0.5 s a cycle: {first:?}"
    );
}

/// A hangar whose `on_enter` spawns a moving gunship with two wings to
/// spawn, carrying its `SceneEntity`.
struct Hangar;

impl GameState for Hangar {
    fn id(&self) -> StateId {
        "hangar"
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        ctx.world.spawn_with(
            RigidBodyBundle::kinematic(
                Position::new(100.0, 100.0),
                Collider::aabb(Vec2::splat(8.0)),
            )
            .with_velocity(Velocity::new(60.0, 0.0))
            .with((
                Transform::from_position(Vec2::new(100.0, 100.0)),
                SpawnChildren::new()
                    .with_child("wing", Vec2::new(-24.0, 0.0))
                    .with_child("wing", Vec2::new(24.0, 0.0)),
                SceneEntity {
                    state_id: ctx.state_id,
                },
            )),
        );
    }

    fn on_exit(&mut self, _ctx: &mut StateContext) {}

    fn update(&mut self, _world: &mut World) {}
}

#[test]
fn spawned_children_follow_their_parent_and_leave_with_its_state() {
    use tungsten_core::{PrefabData, PrefabRegistry, Tag};

    let dir = std::env::temp_dir().join(format!("tungsten_kit_wings_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("wing.json");
    std::fs::write(&path, r#"{ "tag": "wing" }"#).unwrap();
    let mut harness = Harness::new(kit_app());
    let wing = PrefabData::load(&path, harness.world()).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    harness
        .world_mut()
        .resource_mut::<PrefabRegistry>()
        .insert("wing".to_string(), wing);
    harness.set_dt(1.0 / 60.0);
    harness.step(1);
    let count = harness.world().entity_count();
    harness
        .world_mut()
        .resource_mut::<StateStack>()
        .request_push(Hangar);
    harness.step(2);
    let gunship = harness
        .world()
        .query::<(Entity, &RigidBody)>()
        .map(|(entity, _)| entity)
        .next()
        .unwrap();
    let wings: Vec<Entity> = harness
        .world()
        .get::<Children>(gunship)
        .expect("the wings joined its list")
        .iter()
        .collect();
    assert_eq!(wings.len(), 2);
    for _ in 0..20 {
        harness.step(1);
        let at = position(&harness, gunship);
        for (&wing, dx) in wings.iter().zip([-24.0, 24.0]) {
            assert!(close(position(&harness, wing), at + Vec2::new(dx, 0.0)));
        }
    }
    let world = harness.world();
    for &wing in &wings {
        assert_eq!(world.get::<Tag>(wing).unwrap().name, "wing");
        assert_eq!(world.get::<Source>(wing), Some(&Source::new(gunship)));
        assert_eq!(world.get::<SceneEntity>(wing).unwrap().state_id, "hangar");
    }
    assert!(position(&harness, gunship).x > 110.0, "it moved");

    pop_state(&mut harness);
    harness.step(1);
    assert_eq!(harness.world().query::<&SceneEntity>().count(), 0);
    assert_eq!(
        harness.world().entity_count(),
        count,
        "nothing of the hangar remains"
    );
}

#[test]
fn sound_on_event_waits_out_its_min_interval() {
    let mut app = App::new(Config::default()).unwrap();
    app.add_plugins(KitPlugins::set());
    app.add_plugin(SoundOnEvent::<Hit>::new("hit_sound", "thud").with_min_interval(0.1));
    let mut harness = Harness::new(app);
    harness.set_dt(1.0 / 60.0);
    harness
        .world_mut()
        .resource_mut::<SoundRegistry>()
        .register(
            "thud".to_string(),
            SoundData {
                samples: vec![0.0; 4],
                sample_rate: 44_100,
                channels: 1,
            },
            0.5,
            false,
        );
    let mut played = Vec::new();
    for frame in 0..13 {
        harness
            .world_mut()
            .resource_mut::<EventQueue<Hit>>()
            .send(Hit);
        harness.step(1);
        if !harness.audio().is_empty() {
            played.push(frame);
        }
    }
    assert_eq!(played, [0, 6, 12], "a hit every frame plays every 0.1 s");

    // Real time runs while the game clock is paused.
    harness.world_mut().resource_mut::<Time>().pause();
    let mut paused = 0;
    for _ in 0..6 {
        harness
            .world_mut()
            .resource_mut::<EventQueue<Hit>>()
            .send(Hit);
        harness.step(1);
        paused += harness.audio().len();
    }
    assert_eq!(paused, 1);
}

/// Hits on the slime for the test systems to land: one in `update`, one in
/// `post_update` after the kit's, one in the fixed step before the cues.
#[derive(Default)]
struct PendingStrikes {
    target: Option<Entity>,
    update: bool,
    post_update: bool,
    fixed: bool,
}

fn strike(world: &mut World, take: fn(&mut PendingStrikes) -> &mut bool) {
    let pending = world.resource_mut::<PendingStrikes>();
    if std::mem::take(take(pending))
        && let Some(target) = pending.target
    {
        damage(world, target, 1.0);
    }
}

fn game_count(harness: &Harness, name: &str) -> i64 {
    harness.world().resource::<Counters>().get(name)
}

#[test]
fn cues_react_once_to_each_event_from_any_stage() {
    let mut app = kit_app();
    app.add_system_named("strike_update", |world: &mut World| {
        strike(world, |p| &mut p.update);
    });
    app.add_system_to(
        Stage::PostUpdate,
        system("strike_late", |world: &mut World| {
            strike(world, |p| &mut p.post_update);
        })
        .after("kit_debug_draw"),
    );
    app.add_system_to(
        Stage::FixedUpdate,
        crate::kit::Phase::Resolve.join(system("strike_fixed", |world: &mut World| {
            strike(world, |p| &mut p.fixed);
        })),
    );
    let mut harness = Harness::new(app);
    // Two fixed steps a frame.
    harness.set_dt(1.0 / 30.0);
    let slime = spawn_at(
        &mut harness,
        0.0,
        0.0,
        (
            Health::new(100.0),
            Cues::new().on("damaged", [CueAction::counter_add("hits", 1).for_game()]),
        ),
    );
    harness.world_mut().insert_resource(PendingStrikes {
        target: Some(slime),
        ..PendingStrikes::default()
    });
    let arms: [fn(&mut PendingStrikes); 3] = [
        |p| p.update = true,
        |p| p.post_update = true,
        |p| p.fixed = true,
    ];
    let mut counts = Vec::new();
    for arm in arms {
        arm(harness.world_mut().resource_mut::<PendingStrikes>());
        harness.step(4);
        counts.push(game_count(&harness, "hits"));
    }
    assert_eq!(counts, [1, 2, 3], "each hit once, whichever stage sent it");
}

#[test]
fn entity_cues_run_before_rules_and_despawns_after_spawns() {
    use crate::kit_recipes::{DTS, recipe, spawn};
    use tungsten_core::Tag;

    for dt in DTS {
        let mut harness = recipe("top_down", KitPlugins::set(), dt);
        let slime = spawn(&mut harness, "slime", [40.0, 40.0]);
        harness.world_mut().insert(
            slime,
            Cues::new().on(
                "killed",
                [
                    CueAction::despawn(),
                    CueAction::counter_set("kills", 10).for_game(),
                    CueAction::prefab("blast"),
                ],
            ),
        );
        harness.step(1);
        assert_eq!(damage(harness.world_mut(), slime, 5.0), Outcome::Killed);
        harness.step(1);
        // The own cue sets 10, then the shared `killed` rule adds 1.
        assert_eq!(game_count(&harness, "kills"), 11, "dt {dt}");
        let world = harness.world();
        assert!(!world.is_alive(slime));
        let blasts: Vec<(Entity, Vec2)> = world
            .query::<(Entity, &Tag, &Transform)>()
            .filter(|(_, tag, _)| tag.name == "blast")
            .map(|(entity, _, transform)| (entity, transform.position))
            .collect();
        assert_eq!(
            blasts.len(),
            1,
            "dt {dt}: the spawn queued before the despawn"
        );
        assert_eq!(blasts[0].1, Vec2::new(40.0, 40.0));
        assert_eq!(world.get::<Source>(blasts[0].0), Some(&Source::new(slime)));
        let order: Vec<String> = world
            .resource::<KitTrace>()
            .about(slime)
            .filter_map(|entry| match &entry.kind {
                TraceKind::Cue { action, rule, .. } => Some(format!("{action} {rule:?}")),
                _ => None,
            })
            .collect();
        assert_eq!(
            order,
            [
                "despawn None",
                "counter None",
                "prefab None",
                "label Some(\"combat.1\")",
                "counter Some(\"combat.0\")"
            ],
            "dt {dt}: every occurrence's own cues, then the hit's rule and the kill's"
        );
    }
}

/// The game's pickup: a coin taken by a hero.
struct PickedUp {
    coin: Entity,
    hero: Entity,
}

/// A coin that picks itself up is no pickup.
fn picked_up(event: &PickedUp) -> Option<CueEvent> {
    (event.coin != event.hero).then(|| {
        CueEvent::new(event.coin, Vec2::ZERO)
            .with_amount(5.0)
            .with_other(event.hero)
    })
}

#[test]
fn a_game_event_reaches_cues_through_cues_on_event() {
    let mut app = kit_app();
    app.add_plugin(CuesOnEvent::<PickedUp>::new("picked_up", picked_up));
    let mut harness = Harness::new(app);
    let hero = spawn_at(&mut harness, 0.0, 0.0, (Visibility::default(),));
    let coin = spawn_at(
        &mut harness,
        8.0,
        0.0,
        (Cues::new().on(
            "picked_up",
            [
                CueAction::counter_add("coins", 5).for_other(),
                CueAction::label(LabelStyle::new("sans")),
                CueAction::despawn(),
            ],
        ),),
    );
    let fake = spawn_at(&mut harness, 16.0, 0.0, (Visibility::default(),));
    let queue = harness.world_mut().resource_mut::<EventQueue<PickedUp>>();
    queue.send(PickedUp { coin, hero });
    queue.send(PickedUp {
        coin: fake,
        hero: fake,
    });
    harness.step(2);
    let world = harness.world();
    assert_eq!(world.get::<Counters>(hero).unwrap().get("coins"), 5);
    assert!(!world.is_alive(coin));
    assert!(world.is_alive(fake), "the map's `None` skipped it");
    harness.step(2);
    assert_eq!(
        harness.world().get::<Counters>(hero).unwrap().get("coins"),
        5,
        "once"
    );
}

/// The frame's drawn labels: text, the pivot's physical point, scale and
/// alpha.
fn drawn_labels(harness: &Harness) -> Vec<(String, [f32; 2], f32, u8)> {
    harness
        .draw()
        .unwrap()
        .world_labels
        .iter()
        .map(|run| (run.content.clone(), run.position, run.scale, run.color[3]))
        .collect()
}

#[test]
fn labels_rise_fade_and_hold_while_paused() {
    let mut harness = Harness::new(kit_app());
    harness.set_dt(0.1);
    let slime = spawn_at(
        &mut harness,
        0.0,
        0.0,
        (
            Health::new(10.0),
            Cues::new().on(
                "damaged",
                [CueAction::label(
                    LabelStyle::new("sans")
                        .with_seconds(1.0)
                        .with_rise(50.0)
                        .with_pop(2.0),
                )],
            ),
        ),
    );
    harness.step(1);
    damage(harness.world_mut(), slime, 2.5);
    let mut frames = Vec::new();
    for _ in 0..4 {
        harness.step(1);
        frames.push(drawn_labels(&harness));
    }
    // The hit's own frame draws it fresh: at its point, popped to 2.
    let first = frames[0][0].clone();
    assert_eq!(first.0, "2.5");
    assert_eq!(first.3, 255);
    for pair in frames.windows(2) {
        let (before, after) = (&pair[0][0], &pair[1][0]);
        assert!(
            after.1[1] < before.1[1],
            "it rises: {before:?} then {after:?}"
        );
    }
    assert!(frames[3][0].2 < first.2, "the pop settles");

    harness.world_mut().resource_mut::<Time>().pause();
    harness.step(1);
    let held = drawn_labels(&harness)[0].clone();
    harness.step(5);
    let paused = drawn_labels(&harness);
    assert_eq!(paused.len(), 1, "still drawn every frame");
    assert_eq!(paused[0].1, held.1, "a paused clock holds it");

    harness.world_mut().resource_mut::<Time>().resume();
    harness.step(3);
    let fading = drawn_labels(&harness);
    assert!(fading[0].3 < 255, "past half its second it fades");
    harness.step(4);
    assert!(drawn_labels(&harness).is_empty(), "gone after its second");
}

#[test]
fn the_trace_tells_why_an_entity_died() {
    let mut harness = Harness::new(kit_app());
    let slime = spawn_at(
        &mut harness,
        0.0,
        0.0,
        (
            Health::new(1.0),
            Cues::new().on("killed", [CueAction::despawn()]),
        ),
    );
    harness.step(1);
    damage(harness.world_mut(), slime, 3.0);
    harness.step(1);
    assert!(!harness.world().is_alive(slime));
    let trace = harness.world().resource::<KitTrace>();
    let head = format!("{slime} ");
    let story: Vec<String> = trace
        .about(slime)
        .map(|entry| {
            let line = entry.to_string();
            line.split_once(&head)
                .map(|(_, rest)| rest.to_owned())
                .unwrap()
        })
        .collect();
    assert_eq!(
        story,
        [
            "damaged 3",
            "killed 3",
            "killed: despawn (own cue)",
            "despawned by cues"
        ],
        "{}",
        trace
            .about(slime)
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    );
    // The hit came between frames; the cue ran in the next frame's step.
    let entries: Vec<_> = trace.about(slime).collect();
    assert_eq!(
        entries[0].step, entries[2].step,
        "pushed outside the steps: the next one"
    );
    assert_eq!(entries[2].frame, entries[0].frame + 1);
}

// M64's motion and facing, each item through the engine's frame.

use std::f32::consts::{FRAC_PI_2, TAU};

use tungsten_core::physics::PrevPosition;
use tungsten_core::{CameraController, Plugin};

use crate::kit::{
    ACTION_INPUT, AIM, ANIMATION_TICK, ActionInputPlugin, Aim, AimPlugin, AnimationPlugin,
    CHARACTER_ANIMATOR, CharacterAnimator, CharacterAnimatorPlugin, DepthSort, FACING, FOLLOW,
    Facing, FacingPlugin, Follow, FollowPlugin, KNOCKBACK, Knockback, KnockbackPlugin,
    LINEAR_MOTION, LinearMotion, LinearMotionPlugin, ORBIT, OSCILLATOR, Orbit, OrbitPlugin,
    Oscillator, OscillatorPlugin, PATH_FOLLOW, PLATFORMER_CONTROLLER, PathFollowerPlugin,
    PlatformerControllerPlugin, SPEED_LIMIT, STEERING, SpeedLimit, SpeedLimitPlugin, StayInView,
    Steering, SteeringPlugin, TOP_DOWN_MOVER, Target, TopDownMoverPlugin, Wrap, knock_back,
};

const MOTION_DTS: [f32; 2] = [1.0 / 60.0, 1.0 / 30.0];

fn motion_harness(dt: f32) -> Harness {
    let mut harness = Harness::new(kit_app());
    harness.set_dt(dt);
    harness
}

/// A bodiless entity at `(x, y)` with `rest`: a `Transform`, a `Position`
/// and a `PrevPosition` there.
fn bodiless(harness: &mut Harness, x: f32, y: f32, rest: impl Bundle) -> Entity {
    let at = Vec2::new(x, y);
    spawn_at(harness, x, y, (Position(at), PrevPosition(at))).also(harness, rest)
}

trait Also {
    fn also(self, harness: &mut Harness, rest: impl Bundle) -> Entity;
}

impl Also for Entity {
    fn also(self, harness: &mut Harness, rest: impl Bundle) -> Entity {
        harness.world_mut().insert_bundle(self, rest);
        self
    }
}

fn body(harness: &Harness, entity: Entity) -> Vec2 {
    harness.world().get::<Position>(entity).unwrap().0
}

/// The fixed steps run since `Time` began.
fn steps_run(harness: &Harness, steps: &mut u32) {
    *steps += harness.world().resource::<Time>().fixed_steps_this_frame();
}

#[test]
fn a_bodiless_mover_is_drawn_between_its_last_two_steps_and_holds_while_paused() {
    let mut harness = motion_harness(0.025);
    let rock = bodiless(
        &mut harness,
        0.0,
        0.0,
        (LinearMotion::new(Vec2::new(60.0, 30.0)),),
    );
    let mut between = 0;
    for _ in 0..12 {
        harness.step(1);
        let world = harness.world();
        let (prev, cur) = (
            world.get::<PrevPosition>(rock).unwrap().0,
            body(&harness, rock),
        );
        let alpha = world.resource::<Time>().alpha();
        let drawn = position(&harness, rock);
        assert!(
            (drawn - (prev + (cur - prev) * alpha)).length() < 1e-4,
            "{drawn}"
        );
        if alpha > 1e-3 && alpha < 1.0 - 1e-3 {
            between += 1;
            assert!(drawn.x > prev.x && drawn.x < cur.x, "{prev} {drawn} {cur}");
        }
    }
    assert!(between > 4);
    harness.world_mut().resource_mut::<Time>().pause();
    let (held, drawn) = (body(&harness, rock), position(&harness, rock));
    harness.step(5);
    assert_eq!(
        (body(&harness, rock), position(&harness, rock)),
        (held, drawn)
    );
}

#[test]
fn an_oscillator_returns_each_period_and_rides_a_path_with_its_whole_wave() {
    for dt in MOTION_DTS {
        let mut harness = motion_harness(dt);
        let gem = bodiless(
            &mut harness,
            100.0,
            100.0,
            (Oscillator::position(Vec2::new(0.0, 6.0), 1.0),),
        );
        let lift = harness.world_mut().spawn_with(
            RigidBodyBundle::kinematic(
                Position::new(300.0, 300.0),
                Collider::aabb(Vec2::new(40.0, 8.0)),
            )
            .with((
                Transform::from_position(Vec2::new(300.0, 300.0)),
                Oscillator::position(Vec2::new(0.0, 24.0), 2.0),
            )),
        );
        let path =
            PathFollower::new([Vec2::new(500.0, 300.0), Vec2::new(620.0, 300.0)], 60.0).ping_pong();
        let rider = harness.world_mut().spawn_with(
            RigidBodyBundle::kinematic(
                Position::new(500.0, 300.0),
                Collider::aabb(Vec2::new(30.0, 6.0)),
            )
            .with((
                Transform::from_position(Vec2::new(500.0, 300.0)),
                path,
                Oscillator::position(Vec2::new(0.0, 10.0), 1.0),
            )),
        );
        let mut steps = 0;
        for second in 1..=4 {
            for _ in 0..(1.0 / dt).round() as u32 {
                harness.step(1);
                steps_run(&harness, &mut steps);
                let t = steps as f32 / 60.0;
                let leg = t % 4.0;
                let x = if leg <= 2.0 {
                    60.0 * leg
                } else {
                    120.0 - 60.0 * (leg - 2.0)
                };
                let expected = Vec2::new(500.0 + x, 300.0 + 10.0 * (TAU * t).sin());
                let at = body(&harness, rider);
                assert!(
                    (at - expected).length() < 1e-2,
                    "dt {dt} t {t}: {at} vs {expected}"
                );
            }
            assert!(
                body(&harness, gem).distance(Vec2::new(100.0, 100.0)) < 1e-3,
                "dt {dt}"
            );
            if second % 2 == 0 {
                let at = body(&harness, lift);
                assert!(
                    at.distance(Vec2::new(300.0, 300.0)) < 1e-2,
                    "dt {dt}: no drift: {at}"
                );
            }
        }
    }
}

#[test]
fn an_oscillators_rotation_and_scale_draw_at_the_drawn_positions_time() {
    let mut harness = motion_harness(0.025);
    let sign = bodiless(
        &mut harness,
        0.0,
        0.0,
        (Oscillator::position(Vec2::new(8.0, 0.0), 1.0)
            .with_rotation(0.4, 0.8)
            .with_scale(0.2, 0.5),),
    );
    let mut steps = 0;
    for _ in 0..20 {
        harness.step(1);
        steps_run(&harness, &mut steps);
        let alpha = harness.world().resource::<Time>().alpha();
        let drawn_time = (steps as f32 - 1.0 + alpha) / 60.0;
        let transform = harness.world().get::<Transform>(sign).unwrap();
        let rotation = 0.4 * (TAU * drawn_time / 0.8).sin();
        let scale = 1.0 + 0.2 * (TAU * drawn_time / 0.5).sin();
        assert!(
            (transform.rotation - rotation).abs() < 1e-4,
            "{} vs {rotation}",
            transform.rotation
        );
        assert!((transform.scale - Vec2::splat(scale)).length() < 1e-4);
        let (prev, cur) = (
            harness.world().get::<PrevPosition>(sign).unwrap().0,
            body(&harness, sign),
        );
        assert!((transform.position - (prev + (cur - prev) * alpha)).length() < 1e-4);
    }
    harness.world_mut().resource_mut::<Time>().pause();
    let held = *harness.world().get::<Transform>(sign).unwrap();
    harness.step(5);
    let now = harness.world().get::<Transform>(sign).unwrap();
    assert_eq!(
        (now.rotation, now.scale),
        (held.rotation, held.scale),
        "held while paused"
    );
}

#[test]
fn a_follower_sits_on_its_target_closes_by_its_factor_and_bobs_about_its_point() {
    let mut harness = motion_harness(1.0 / 60.0);
    let target = bodiless(
        &mut harness,
        0.0,
        0.0,
        (LinearMotion::new(Vec2::new(50.0, 20.0)),),
    );
    let sitter = bodiless(
        &mut harness,
        0.0,
        0.0,
        (
            Follow::new(Target::source()).with_offset(Vec2::new(0.0, -16.0)),
            Source::new(target),
        ),
    );
    let lagger = bodiless(
        &mut harness,
        -100.0,
        0.0,
        (
            Follow::new(Target::source()).with_lag(0.2),
            Source::new(target),
        ),
    );
    let bobber = bodiless(
        &mut harness,
        0.0,
        0.0,
        (
            Follow::new(Target::source()).with_offset(Vec2::new(20.0, 0.0)),
            Oscillator::position(Vec2::new(0.0, 5.0), 0.5),
            Source::new(target),
        ),
    );
    let factor = (-(1.0_f32 / 60.0) / 0.2).exp();
    for n in 1..=60 {
        let before = body(&harness, lagger);
        harness.step(1);
        let goal = body(&harness, target);
        assert!((body(&harness, sitter) - (goal + Vec2::new(0.0, -16.0))).length() < 1e-3);
        let gap = goal - body(&harness, lagger);
        assert!(
            (gap - (goal - before) * factor).length() < 1e-3,
            "{n}: {gap}"
        );
        let bob = Vec2::new(20.0, 5.0 * (TAU * n as f32 / 60.0 / 0.5).sin());
        assert!(
            (body(&harness, bobber) - (goal + bob)).length() < 1e-3,
            "{n}"
        );
    }
}

#[test]
fn an_orbiter_keeps_its_radius_from_a_moving_kinematic_centres_drawn_point() {
    let mut harness = motion_harness(0.025);
    let centre = harness.world_mut().spawn_with(
        RigidBodyBundle::kinematic(
            Position::new(100.0, 100.0),
            Collider::aabb(Vec2::splat(8.0)),
        )
        .with((
            Transform::from_position(Vec2::new(100.0, 100.0)),
            LinearMotion::new(Vec2::new(80.0, 40.0)),
        )),
    );
    let blade = bodiless(
        &mut harness,
        100.0,
        100.0,
        (
            Orbit::new(48.0, 3.0).around(Target::source()),
            Source::new(centre),
        ),
    );
    harness.step(2);
    for _ in 0..40 {
        harness.step(1);
        let gap = position(&harness, blade).distance(position(&harness, centre));
        assert!((gap - 48.0).abs() < 0.05, "{gap}");
    }
    assert!(position(&harness, centre).x > 150.0, "the centre moved");
}

#[test]
fn steering_keeps_a_walls_push_takes_its_stat_and_arrives_on_a_tight_radius() {
    // A dynamic seeker against a wall: the solver's push stays.
    let mut harness = motion_harness(1.0 / 60.0);
    let goal = Target::point(Vec2::new(400.0, 0.0));
    harness.world_mut().spawn_with(RigidBodyBundle::r#static(
        Position::new(110.0, 0.0),
        Collider::aabb(Vec2::new(10.0, 300.0)),
    ));
    let brute = harness.world_mut().spawn_with(
        RigidBodyBundle::dynamic(Position::new(0.0, 0.0), Collider::circle(8.0)).with((
            Transform::default(),
            Steering::seek(goal.clone(), 90.0).with_acceleration(900.0),
        )),
    );
    let slowed = bodiless(
        &mut harness,
        0.0,
        200.0,
        (
            Steering::seek(goal.clone(), 90.0),
            Stats::new().with(Stats::MOVE_SPEED, 30.0),
        ),
    );
    for _ in 0..120 {
        harness.step(1);
        assert!(
            body(&harness, brute).x <= 92.0 + 0.5,
            "{}",
            body(&harness, brute)
        );
    }
    let speed = harness
        .world()
        .get::<Steering>(slowed)
        .unwrap()
        .velocity()
        .length();
    assert!((speed - 30.0).abs() < 1e-3, "the stat's top speed: {speed}");

    for dt in MOTION_DTS {
        let mut harness = motion_harness(dt);
        let landing = Vec2::new(200.0, 50.0);
        let tight = bodiless(
            &mut harness,
            0.0,
            0.0,
            (Steering::arrive(Target::point(landing), 120.0).with_radius(0.5),),
        );
        // A kinematic seeker at full speed, its velocity the body's.
        let late = harness.world_mut().spawn_with(
            RigidBodyBundle::kinematic(Position::new(300.0, 0.0), Collider::aabb(Vec2::splat(4.0)))
                .with((
                    Transform::from_position(Vec2::new(300.0, 0.0)),
                    Steering::seek(Target::point(Vec2::new(900.0, 0.0)), 100.0),
                )),
        );
        harness.step(1);
        assert_eq!(
            harness.world().get::<Velocity>(late).unwrap().0,
            Vec2::new(100.0, 0.0)
        );
        // Switched to arrive a pixel short of its target, braking at 60 px/s²:
        // a step's braking leaves it far over the pixel.
        let stop = body(&harness, late) + Vec2::X;
        harness.world_mut().insert(
            late,
            Steering::arrive(Target::point(stop), 100.0).with_acceleration(60.0),
        );
        harness.step(1);
        assert!(
            body(&harness, late).distance(stop) < 1e-3,
            "dt {dt}: an emergency stop"
        );
        harness.step((4.0 / dt) as u32);
        for _ in 0..20 {
            harness.step(1);
            assert!(
                body(&harness, tight).distance(landing) < 1e-3,
                "dt {dt}: lands and stays"
            );
            assert!(
                body(&harness, late).distance(stop) < 1e-3,
                "dt {dt}: and stays"
            );
        }
    }
}

#[test]
fn knockback_slides_along_walls_stops_a_driverless_body_and_launches_a_platformer() {
    let mut harness = motion_harness(1.0 / 60.0);
    // A wall whose left face is at x 100; a top-down mover on its body
    // beside it, walking down.
    harness.world_mut().spawn_with(RigidBodyBundle::r#static(
        Position::new(110.0, 0.0),
        Collider::aabb(Vec2::new(10.0, 500.0)),
    ));
    let mut walker = TopDownMover::new(60.0);
    walker.set_direction(Vec2::Y);
    let brawler = harness.world_mut().spawn_with(
        RigidBodyBundle::kinematic(Position::new(80.0, 0.0), Collider::aabb(Vec2::splat(10.0)))
            .with((
                Transform::from_position(Vec2::new(80.0, 0.0)),
                walker,
                Knockback::new(0.3),
            )),
    );
    let crate_ = harness.world_mut().spawn_with(
        RigidBodyBundle::kinematic(Position::new(-300.0, 0.0), Collider::aabb(Vec2::splat(8.0)))
            .with((
                Transform::from_position(Vec2::new(-300.0, 0.0)),
                Knockback::new(0.2),
            )),
    );
    harness.step(2);
    assert!(knock_back(
        harness.world_mut(),
        brawler,
        Vec2::new(900.0, -300.0)
    ));
    assert!(knock_back(
        harness.world_mut(),
        crate_,
        Vec2::new(-240.0, 0.0)
    ));
    for _ in 0..40 {
        harness.step(1);
        assert!(
            body(&harness, brawler).x <= 90.0 + 1e-3,
            "never into the wall"
        );
    }
    assert!(
        (body(&harness, brawler).x - 90.0).abs() < 0.05,
        "against it"
    );
    // 240 px/s falling to 0 over 0.2 s: 24 px, then still.
    assert!(
        (body(&harness, crate_).x + 324.0).abs() < 1e-2,
        "{}",
        body(&harness, crate_)
    );
    assert_eq!(
        harness.world().get::<Velocity>(crate_).unwrap().0,
        Vec2::ZERO
    );

    let mut harness = motion_harness(1.0 / 60.0);
    harness.world_mut().insert_resource(PhysicsConfig {
        gravity: Vec2::new(0.0, GRAVITY),
        ..PhysicsConfig::default()
    });
    tile_map(&mut harness, 60, floor_row(60));
    let acrobat = hero(
        &mut harness,
        Vec2::new(300.0, 100.0),
        PlatformerController::new(RUN, JUMP),
    );
    harness.world_mut().insert(acrobat, Knockback::new(0.25));
    harness.step(20);
    assert!(pc_ref(&harness, acrobat).grounded());
    assert!(knock_back(
        harness.world_mut(),
        acrobat,
        Vec2::new(-200.0, -500.0)
    ));
    pc(&mut harness, acrobat).set_run(1.0);
    harness.step(5);
    let thrown = harness.world().get::<Velocity>(acrobat).unwrap().0;
    assert!(
        (thrown.x + 200.0).abs() < 1e-3,
        "launched, the run locked out: {thrown}"
    );
    assert!(!pc_ref(&harness, acrobat).can_jump());
    harness.step(20);
    assert!(
        harness.world().get::<Velocity>(acrobat).unwrap().0.x > 0.0,
        "the lock ended"
    );
}

/// The y velocity each limited entity entered the physics step with, read
/// between the speed limit and the step.
#[derive(Default)]
struct Entering(Vec<f32>);

#[test]
fn a_speed_limit_bounds_free_fall_caps_a_knocked_body_and_never_lets_a_step_pass_it() {
    const CAP: f32 = 300.0;
    const FALL: f32 = 1200.0;
    let mut app = kit_app();
    app.add_system_to(
        Stage::FixedUpdate,
        system("entering", |world: &mut World| {
            let speeds: Vec<f32> = world
                .query::<(&SpeedLimit, &Velocity)>()
                .map(|(_, velocity)| velocity.0.y)
                .collect();
            world.resource_mut::<Entering>().0.extend(speeds);
        })
        .after(SPEED_LIMIT)
        .before(tungsten_core::physics::PHYSICS_STEP),
    );
    let mut harness = Harness::new(app);
    harness.world_mut().insert_resource(Entering::default());
    harness.world_mut().insert_resource(PhysicsConfig {
        gravity: Vec2::new(0.0, FALL),
        ..PhysicsConfig::default()
    });
    let crate_ = harness.world_mut().spawn_with(
        RigidBodyBundle::dynamic(Position::new(0.0, 0.0), Collider::aabb(Vec2::splat(8.0))).with((
            Transform::default(),
            SpeedLimit::y(CAP),
            Knockback::new(0.1),
        )),
    );
    let cart = harness.world_mut().spawn_with(
        RigidBodyBundle::kinematic(Position::new(200.0, 0.0), Collider::aabb(Vec2::splat(4.0)))
            .with((
                Transform::from_position(Vec2::new(200.0, 0.0)),
                LinearMotion::new(Vec2::new(500.0, 0.0)).with_acceleration(Vec2::new(0.0, 900.0)),
                SpeedLimit::speed(240.0),
            )),
    );
    let rock = bodiless(
        &mut harness,
        400.0,
        0.0,
        (
            LinearMotion::new(Vec2::new(0.0, 900.0)),
            SpeedLimit::x(100.0).with_y(CAP),
        ),
    );
    let dt = 1.0 / 60.0;
    for n in 0..120 {
        if n == 60 {
            assert!(knock_back(
                harness.world_mut(),
                crate_,
                Vec2::new(0.0, 3000.0)
            ));
        }
        let (crate_at, cart_at, rock_at) = (
            body(&harness, crate_),
            body(&harness, cart),
            body(&harness, rock),
        );
        harness.step(1);
        let fell = body(&harness, crate_).y - crate_at.y;
        assert!(fell <= (CAP + FALL * dt) * dt + 1e-3, "{n}: {fell}");
        assert!(
            harness.world().get::<Velocity>(crate_).unwrap().0.y <= CAP + 1e-3,
            "{n}: leaves capped"
        );
        assert!((body(&harness, cart) - cart_at).length() <= 240.0 * dt + 1e-3);
        assert!(body(&harness, rock).y - rock_at.y <= CAP * dt + 1e-3);
    }
    let entering = &harness.world().resource::<Entering>().0;
    // The crate and the cart, which have a `Velocity`, each step.
    assert_eq!(entering.len(), 240);
    assert!(
        entering.iter().all(|&y| y <= CAP + 1e-3),
        "every step enters capped"
    );
}

#[test]
fn the_frame_after_a_wrap_draws_the_entity_at_the_far_side() {
    let mut harness = motion_harness(0.025);
    let rock = bodiless(
        &mut harness,
        600.0,
        100.0,
        (
            LinearMotion::new(Vec2::new(300.0, 0.0)),
            Wrap::region(Vec2::ZERO, Vec2::new(640.0, 360.0)),
        ),
    );
    let mut wraps = 0;
    for _ in 0..100 {
        let (before, drawn_before) = (body(&harness, rock).x, position(&harness, rock).x);
        harness.step(1);
        if body(&harness, rock).x < before {
            // The frame after a wrap: drawn at the far side, never between.
            let drawn = position(&harness, rock).x;
            assert!(
                drawn_before > 560.0 && drawn < 40.0,
                "{drawn_before} then {drawn}"
            );
            wraps += 1;
        }
    }
    assert_eq!(
        wraps, 2,
        "750 px at 300 px/s for 2.5 s: out at 40 px and 680 px"
    );
}

#[test]
fn a_camera_shake_does_not_move_a_clamped_entity() {
    let mut harness = motion_harness(1.0 / 60.0);
    {
        let camera = harness.world_mut().resource_mut::<CameraController>();
        camera.shake_max_offset = Vec2::splat(20.0);
        camera.shake_frequency_hz = 9.0;
        camera.shake_decay = 0.0;
        camera.add_trauma(1.0);
    }
    let mut mover = TopDownMover::new(300.0);
    mover.set_direction(Vec2::X);
    let fighter = bodiless(
        &mut harness,
        1000.0,
        300.0,
        (mover, StayInView::view(8.0).with_size(Vec2::splat(24.0))),
    );
    harness.step(60);
    let held = body(&harness, fighter);
    assert_eq!(held.x, 1280.0 - 8.0 - 24.0);
    let mut shaken = 0;
    for _ in 0..30 {
        harness.step(1);
        let camera = harness.world().resource::<CameraState>().position;
        shaken += usize::from(camera != Vec2::ZERO);
        assert_eq!(body(&harness, fighter), held, "the shake moves nothing");
    }
    assert!(shaken > 20, "the camera shook");
}

#[test]
fn an_aim_follows_the_cursor_under_a_moved_camera_and_a_loose_nearest_fails_the_load() {
    let mut harness = motion_harness(1.0 / 60.0);
    {
        let camera = harness.world_mut().resource_mut::<CameraState>();
        camera.position = Vec2::new(1000.0, 500.0);
        camera.zoom = 2.0;
    }
    let turret = bodiless(
        &mut harness,
        1000.0,
        500.0,
        (Aim::cursor().with_offset(Vec2::new(16.0, 16.0)),),
    );
    harness.set_cursor(200.0, 32.0);
    harness.step(1);
    // The cursor's world point: the camera's corner plus 200 by 32 over 2.
    let toward = (Vec2::new(1100.0, 516.0) - Vec2::new(1016.0, 516.0)).normalize();
    assert_eq!(
        harness.world().get::<Aim>(turret).unwrap().direction(),
        Some(toward)
    );

    let dir = std::env::temp_dir().join(format!("tungsten_kit_loose_aim_{}", std::process::id()));
    std::fs::create_dir_all(dir.join("prefabs")).unwrap();
    std::fs::write(
        dir.join("manifest.json"),
        r#"{ "prefabs": { "sentry": { "path": "prefabs/sentry.json" } } }"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("prefabs/sentry.json"),
        r#"{ "components": { "aim": { "from": "target", "target": { "nearest": { "none": ["boss"] } } } } }"#,
    )
    .unwrap();
    let mut harness = motion_harness(1.0 / 60.0);
    let error = harness
        .load_manifest(dir.join("manifest.json"))
        .unwrap_err();
    std::fs::remove_dir_all(&dir).unwrap();
    let text = format!("{error:#}");
    assert!(text.contains("aim") && text.contains("target"), "{text}");
    assert!(text.contains("no `all` or `any` tag"), "{text}");
}

#[test]
fn turning_writes_a_bodys_heading_and_a_stopped_entity_keeps_facing() {
    let mut harness = motion_harness(1.0 / 60.0);
    let ship = harness.world_mut().spawn_with(
        RigidBodyBundle::kinematic(Position::new(0.0, 0.0), Collider::aabb(Vec2::splat(8.0))).with(
            (
                Transform::default(),
                LinearMotion::new(Vec2::new(0.0, 50.0)),
                Facing::new().turning(FRAC_PI_2),
            ),
        ),
    );
    harness.step(2);
    let turned = harness.world().get::<Transform>(ship).unwrap().rotation;
    assert!((turned - std::f32::consts::PI).abs() < 1e-5, "{turned}");
    harness
        .world_mut()
        .get_mut::<LinearMotion>(ship)
        .unwrap()
        .set_velocity(Vec2::ZERO);
    harness.step(5);
    assert_eq!(
        harness.world().get::<Facing>(ship).unwrap().direction(),
        Vec2::Y
    );
    assert!(
        (harness.world().get::<Transform>(ship).unwrap().rotation - std::f32::consts::PI).abs()
            < 1e-5
    );
}

#[test]
fn a_misspelt_clip_fails_the_load_and_a_new_clip_shows_its_first_frame_however_short() {
    let dir = std::env::temp_dir().join(format!("tungsten_kit_bad_clip_{}", std::process::id()));
    std::fs::create_dir_all(dir.join("prefabs")).unwrap();
    std::fs::create_dir_all(dir.join("animations")).unwrap();
    std::fs::write(
        dir.join("manifest.json"),
        r#"{ "prefabs": { "walker": { "path": "prefabs/walker.json" } }, "animations": { "walk_right": { "path": "animations/walk_right.json" } } }"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("animations/walk_right.json"),
        r#"{ "frames": [{ "duration_ms": 100, "sprite": "walk_0" }], "looping": true }"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("prefabs/walker.json"),
        r#"{ "components": { "character_animator": { "idle": "walk_right", "move": { "left": "walk_lft", "right": "walk_right" } } } }"#,
    )
    .unwrap();
    let mut harness = motion_harness(1.0 / 60.0);
    let error = harness
        .load_manifest(dir.join("manifest.json"))
        .unwrap_err();
    std::fs::remove_dir_all(&dir).unwrap();
    let text = format!("{error:#}");
    assert!(
        text.contains("move.left") && text.contains("walk_lft"),
        "{text}"
    );

    let mut harness = motion_harness(0.05);
    let clip = |first: u32, ms: u32| AnimationData {
        looping: true,
        frames: (0..3)
            .map(|i| AnimationFrame {
                sprite: SpriteAssetId::new(first + i),
                duration_ms: ms,
            })
            .collect(),
    };
    let mut clips = AnimationRegistry::new();
    clips.insert("idle".into(), clip(0, 100));
    clips.insert("dash".into(), clip(70, 10));
    harness.world_mut().insert_resource(clips);
    let runner = bodiless(
        &mut harness,
        0.0,
        0.0,
        (
            CharacterAnimator::new("idle").with_move("dash"),
            LinearMotion::new(Vec2::ZERO),
            Sprite::new(SpriteAssetId::new(99)),
        ),
    );
    harness.step(2);
    assert_eq!(
        harness.world().get::<Sprite>(runner).unwrap().asset_id,
        SpriteAssetId::new(0)
    );
    harness
        .world_mut()
        .get_mut::<LinearMotion>(runner)
        .unwrap()
        .set_velocity(Vec2::new(80.0, 0.0));
    harness.step(1);
    assert_eq!(
        harness.world().get::<Sprite>(runner).unwrap().asset_id,
        SpriteAssetId::new(70),
        "its 10 ms first frame shows on the 50 ms frame that picks it"
    );
    harness.step(1);
    assert_ne!(
        harness.world().get::<Sprite>(runner).unwrap().asset_id,
        SpriteAssetId::new(70)
    );
}

#[test]
fn depth_sorting_draws_the_lower_of_two_crossing_entities_later() {
    let mut harness = motion_harness(1.0 / 60.0);
    let quad = harness
        .world_mut()
        .resource_mut::<AssetRegistry>()
        .register_sprite(
            "quad".into(),
            FilterMode::Nearest,
            16,
            16,
            PathBuf::from("test/quad.png"),
            TextureHandle(0),
            UvRect::FULL,
            None,
            None,
            None,
        );
    let still = spawn_at(
        &mut harness,
        100.0,
        100.0,
        (
            Sprite::new(quad),
            DepthSort::new().with_offset(16.0),
            Visibility::default(),
        ),
    );
    let walker = bodiless(
        &mut harness,
        140.0,
        40.0,
        (
            Sprite::new(quad),
            DepthSort::new().with_offset(16.0),
            Visibility::default(),
            LinearMotion::new(Vec2::new(0.0, 60.0)),
        ),
    );
    let order = |harness: &Harness| -> Vec<f32> {
        harness
            .draw()
            .unwrap()
            .sprites
            .iter()
            .flat_map(|batch| batch.instances.iter().map(|sprite| sprite.position[0]))
            .collect()
    };
    harness.step(1);
    assert_eq!(
        order(&harness),
        [140.0, 100.0],
        "above it, the walker draws first"
    );
    harness.step(120);
    assert!(position(&harness, walker).y > 100.0);
    assert_eq!(
        order(&harness),
        [100.0, 140.0],
        "below it, the walker draws later"
    );
    let _ = still;
}

/// Builds the plugin whose system is `name`.
fn build_owner(name: &str, schedule: &mut tungsten_core::Schedule, world: &mut World) {
    match name {
        ACTION_INPUT => ActionInputPlugin.build(schedule, world),
        AIM => AimPlugin.build(schedule, world),
        LINEAR_MOTION => LinearMotionPlugin.build(schedule, world),
        OSCILLATOR => OscillatorPlugin.build(schedule, world),
        PATH_FOLLOW => PathFollowerPlugin.build(schedule, world),
        PLATFORMER_CONTROLLER => PlatformerControllerPlugin.build(schedule, world),
        TOP_DOWN_MOVER => TopDownMoverPlugin.build(schedule, world),
        STEERING => SteeringPlugin.build(schedule, world),
        FOLLOW => FollowPlugin.build(schedule, world),
        ORBIT => OrbitPlugin.build(schedule, world),
        KNOCKBACK => KnockbackPlugin.build(schedule, world),
        SPEED_LIMIT => SpeedLimitPlugin.build(schedule, world),
        FACING => FacingPlugin.build(schedule, world),
        ANIMATION_TICK => AnimationPlugin.build(schedule, world),
        CHARACTER_ANIMATOR => CharacterAnimatorPlugin.build(schedule, world),
        other => panic!("no plugin for {other}"),
    }
}

/// M64's within-phase edges, each `(earlier, later)`.
const MOTION_EDGES: [(&str, &str); 36] = [
    (ACTION_INPUT, AIM),
    (LINEAR_MOTION, PLATFORMER_CONTROLLER),
    (LINEAR_MOTION, TOP_DOWN_MOVER),
    (LINEAR_MOTION, OSCILLATOR),
    (OSCILLATOR, PLATFORMER_CONTROLLER),
    (OSCILLATOR, TOP_DOWN_MOVER),
    (PATH_FOLLOW, FOLLOW),
    (LINEAR_MOTION, FOLLOW),
    (OSCILLATOR, FOLLOW),
    (PLATFORMER_CONTROLLER, FOLLOW),
    (TOP_DOWN_MOVER, FOLLOW),
    (STEERING, FOLLOW),
    (PATH_FOLLOW, ORBIT),
    (LINEAR_MOTION, ORBIT),
    (OSCILLATOR, ORBIT),
    (PLATFORMER_CONTROLLER, ORBIT),
    (TOP_DOWN_MOVER, ORBIT),
    (STEERING, ORBIT),
    (PATH_FOLLOW, KNOCKBACK),
    (LINEAR_MOTION, KNOCKBACK),
    (OSCILLATOR, KNOCKBACK),
    (TOP_DOWN_MOVER, KNOCKBACK),
    (STEERING, KNOCKBACK),
    (FOLLOW, KNOCKBACK),
    (ORBIT, KNOCKBACK),
    (PATH_FOLLOW, SPEED_LIMIT),
    (LINEAR_MOTION, SPEED_LIMIT),
    (OSCILLATOR, SPEED_LIMIT),
    (PLATFORMER_CONTROLLER, SPEED_LIMIT),
    (TOP_DOWN_MOVER, SPEED_LIMIT),
    (STEERING, SPEED_LIMIT),
    (FOLLOW, SPEED_LIMIT),
    (ORBIT, SPEED_LIMIT),
    (KNOCKBACK, SPEED_LIMIT),
    (FACING, CHARACTER_ANIMATOR),
    (ANIMATION_TICK, CHARACTER_ANIMATOR),
];

#[test]
fn each_motion_edge_holds_with_its_two_plugins_alone_in_either_order_and_reversed_in_the_set() {
    for (earlier, later) in MOTION_EDGES {
        for (first, second) in [(earlier, later), (later, earlier)] {
            let mut world = World::new();
            let mut schedule = tungsten_core::Schedule::new();
            build_owner(first, &mut schedule, &mut world);
            build_owner(second, &mut schedule, &mut world);
            schedule.resolve().unwrap();
            let stage = schedule.stage_of(earlier).unwrap();
            let names: Vec<&str> = schedule.names(stage).collect();
            let at = |name: &str| names.iter().position(|n| *n == name).unwrap();
            assert!(
                at(earlier) < at(later),
                "{earlier} before {later}, built {first} first: {names:?}"
            );
        }
    }
    let mut app = App::new(Config::default()).unwrap();
    app.add_plugins(crate::kit_recipes::motion_reversed());
    app.resolve_schedule().unwrap();
    let text = app.schedule().resolved_text();
    for (earlier, later) in MOTION_EDGES {
        let line = text
            .lines()
            .find(|line| {
                line.contains(&format!(" {earlier},")) || line.ends_with(&format!(" {earlier}"))
            })
            .unwrap();
        let names: Vec<&str> = line.split_once(": ").unwrap().1.split(", ").collect();
        let at = |name: &str| names.iter().position(|n| *n == name).unwrap();
        assert!(
            at(earlier) < at(later),
            "reversed in the set: {earlier} before {later}"
        );
    }
}
