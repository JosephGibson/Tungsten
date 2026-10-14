//! The kit's harness tests: each item through the engine's frame on
//! `Harness`, with `DefaultPlugins` and the kit's plugins added after them.

use glam::Vec2;
use tungsten_core::assets::AnimationFrame;
use tungsten_core::physics::{Collider, Position, RigidBody, RigidBodyBundle, Velocity};
use tungsten_core::{
    AnimationData, AnimationRegistry, AnimationState, AudioCommand, AudioHandle, Bundle,
    CameraState, CommandBuffer, Config, Easing, Entity, EventQueue, SoundData, SoundRegistry,
    Sprite, SpriteAssetId, Stage, Time, Transform, Tween, TweenChannel, WindowSize, World, system,
};

use crate::App;
use crate::kit::{
    AnimationPlugin, Children, DespawnOutside, DespawnOutsidePlugin, HierarchyPlugin, Lifetime,
    LifetimePlugin, LocalOffset, Parent, SoundOnEvent, Spawner, SpawnerPlugin, cursor_to_world,
    despawn_recursive, spawn_child,
};
use crate::testing::Harness;

/// `post_update` with `DefaultPlugins`, then the hierarchy, animation,
/// lifetime, despawn-outside, sound and spawner plugins added in that order.
const KIT_POST_UPDATE: &str = "post_update: physics_sync, transform_propagate, particle_count_refresh, particle_emit, particle_tick, tween_tick, squash_stretch_trigger, squash_stretch_tick, shake_tick, transform_propagate_late, camera_update, animation_tick, lifetime_tick, despawn_outside, hit_sound, spawner_tick";

/// The event the sound plugin plays.
struct Hit;

/// Hits for the test systems to send: one in `update`, one in `post_update`
/// after the sound plugin's system.
#[derive(Default)]
struct PendingHits {
    early: bool,
    late: bool,
}

fn kit_app() -> App {
    let mut app = App::new(Config::default()).unwrap();
    app.add_plugin(HierarchyPlugin);
    app.add_plugin(AnimationPlugin);
    app.add_plugin(LifetimePlugin);
    app.add_plugin(DespawnOutsidePlugin);
    app.add_plugin(SoundOnEvent::<Hit>::new("hit_sound", "thud"));
    app.add_plugin(SpawnerPlugin);
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
    // The other stages are `DefaultPlugins`' own.
    assert_eq!(
        text.lines().nth(2),
        Some("fixed_update: physics_prev_snapshot, physics_step")
    );
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
