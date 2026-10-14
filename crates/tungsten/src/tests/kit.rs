//! The kit's harness tests: each item through the engine's frame on
//! `Harness`, with `DefaultPlugins` and the kit's plugins added after them.

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
    AnimationPlugin, Children, Damaged, DespawnOutside, DespawnOutsidePlugin, HEALTH_TICK,
    HIT_FLASH_MATERIAL, Health, HealthPlugin, HierarchyPlugin, Hit as Outcome, HitFlash, Lifetime,
    LifetimePlugin, LocalOffset, Parent, PathFollower, PathFollowerPlugin, PlatformerController,
    PlatformerControllerPlugin, SoundOnEvent, Spawner, SpawnerPlugin, StateMachine,
    StateMachinePlugin, TopDownMover, TopDownMoverPlugin, TriggerEntered, TriggerZone,
    cursor_to_world, damage, despawn_recursive, spawn_child,
};
use crate::testing::Harness;

/// `post_update` with `DefaultPlugins`, then the hierarchy, animation,
/// lifetime, despawn-outside, sound, spawner and health plugins added in
/// that order.
const KIT_POST_UPDATE: &str = "post_update: physics_sync, transform_propagate, particle_count_refresh, particle_emit, particle_tick, tween_tick, squash_stretch_trigger, squash_stretch_tick, shake_tick, transform_propagate_late, camera_update, animation_tick, lifetime_tick, despawn_outside, hit_sound, spawner_tick, hit_flash_tick";

/// `fixed_update` with `DefaultPlugins`, then the controllers, health, path,
/// zone and state-machine plugins added in that order (`D-158`).
const KIT_FIXED_UPDATE: &str = "fixed_update: physics_prev_snapshot, health_tick, phases, path_follow, platformer_controller, top_down_mover, physics_step, zone_entered";

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
    app.add_plugin(HierarchyPlugin);
    app.add_plugin(AnimationPlugin);
    app.add_plugin(LifetimePlugin);
    app.add_plugin(DespawnOutsidePlugin);
    app.add_plugin(SoundOnEvent::<Hit>::new("hit_sound", "thud"));
    app.add_plugin(SpawnerPlugin);
    app.add_plugin(PlatformerControllerPlugin);
    app.add_plugin(TopDownMoverPlugin);
    app.add_plugin(HealthPlugin);
    app.add_plugin(PathFollowerPlugin);
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
fn kit_plugins_resolve_between_the_sync_and_the_readers() {
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
    harness.set_dt(0.05);
    let bolt = spawn_at(&mut harness, 0.0, 0.0, (Lifetime::new(0.12),));
    let trail = spawn_child(
        harness.world_mut(),
        bolt,
        Vec2::new(-4.0, 0.0),
        (Sprite::new(SpriteAssetId::new(0)),),
    );

    harness.step(2);
    assert!(harness.world().is_alive(bolt) && harness.world().is_alive(trail));

    harness.world_mut().resource_mut::<Time>().pause();
    harness.step(10);
    assert!(
        harness.world().is_alive(bolt),
        "a paused clock holds the lifetime"
    );

    harness.world_mut().resource_mut::<Time>().resume();
    harness.step(1);
    assert!(!harness.world().is_alive(bolt));
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
    harness.set_dt(0.05);
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

    harness.step(1);
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
    harness.step(4);
    assert_eq!(slimes(&harness).len(), 3);
    harness.step(10);
    assert_eq!(slimes(&harness).len(), 3, "the count is spent");
    assert_eq!(
        harness.world().get::<Spawner>(nest).unwrap().remaining(),
        Some(0)
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
