use tungsten_core::{
    CommandBuffer, DeltaTime, Easing, EventQueue, Sprite, Transform, Tween, TweenChannel,
    TweenComplete, TweenDirection, TweenRepeat, Visibility, World,
};

use crate::tweens::tween_tick_system;

fn make_world(dt: f32) -> World {
    let mut world = World::new();
    let mut delta = DeltaTime::new();
    delta.dt = dt;
    world.insert_resource(delta);
    world.insert_resource(CommandBuffer::new());
    world.insert_resource(EventQueue::<TweenComplete>::new());
    world
}

fn flush(world: &mut World) {
    let buf = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush(buf);
    world.insert_resource(CommandBuffer::new());
}

fn spawn_fading_sprite(world: &mut World, tween: Tween) -> tungsten_core::Entity {
    let entity = world.spawn();
    world.insert(entity, Transform::default());
    world.insert(entity, Sprite::new("test"));
    world.insert(entity, Visibility::default());
    world.insert(entity, tween);
    entity
}

#[test]
fn tween_once_completes_and_removes_component() {
    let mut world = make_world(0.03);
    let tween =
        Tween::new(0.1, Easing::Linear).with_channel(TweenChannel::ColorA { from: 0, to: 255 });
    let entity = spawn_fading_sprite(&mut world, tween);

    for _ in 0..5 {
        tween_tick_system(&mut world);
    }
    flush(&mut world);

    let sprite = world.get::<Sprite>(entity).expect("sprite survives");
    assert_eq!(sprite.color[3], 255);
    assert!(world.get::<Tween>(entity).is_none(), "tween removed");

    let events = world.get_resource::<EventQueue<TweenComplete>>().unwrap();
    let count = events.iter().filter(|e| e.entity == entity).count();
    assert_eq!(count, 1, "exactly one TweenComplete");
}

#[test]
fn tween_times_fires_once_after_n_cycles() {
    let mut world = make_world(0.05);
    let tween = Tween::new(0.1, Easing::Linear)
        .with_channel(TweenChannel::ColorA { from: 0, to: 255 })
        .with_repeat(TweenRepeat::Times(3));
    let entity = spawn_fading_sprite(&mut world, tween);

    for _ in 0..8 {
        tween_tick_system(&mut world);
    }
    flush(&mut world);

    assert!(world.get::<Tween>(entity).is_none(), "tween removed");
    let events = world.get_resource::<EventQueue<TweenComplete>>().unwrap();
    let count = events.iter().filter(|e| e.entity == entity).count();
    assert_eq!(count, 1, "Times(3) sends exactly one TweenComplete");
}

#[test]
fn tween_loop_never_completes() {
    let mut world = make_world(0.03);
    let tween = Tween::new(0.1, Easing::Linear)
        .with_channel(TweenChannel::ColorA { from: 0, to: 255 })
        .with_repeat(TweenRepeat::Loop);
    let entity = spawn_fading_sprite(&mut world, tween);

    for _ in 0..20 {
        tween_tick_system(&mut world);
    }
    flush(&mut world);

    assert!(world.get::<Tween>(entity).is_some(), "loop tween persists");
    let events = world.get_resource::<EventQueue<TweenComplete>>().unwrap();
    assert_eq!(
        events.iter().filter(|e| e.entity == entity).count(),
        0,
        "Loop never completes"
    );
}

#[test]
fn tween_pingpong_reverses_at_boundary() {
    let mut world = make_world(0.12);
    let tween = Tween::new(0.1, Easing::Linear)
        .with_channel(TweenChannel::ColorA { from: 0, to: 255 })
        .with_repeat(TweenRepeat::PingPong);
    let entity = spawn_fading_sprite(&mut world, tween);

    tween_tick_system(&mut world);

    let state = world.get::<Tween>(entity).expect("tween retained");
    assert_eq!(state.direction, TweenDirection::Backward);
    assert_eq!(state.elapsed, 0.1, "clamped to duration boundary");
    let events = world.get_resource::<EventQueue<TweenComplete>>().unwrap();
    assert_eq!(
        events.iter().filter(|e| e.entity == entity).count(),
        0,
        "PingPong does not complete"
    );
}

#[test]
fn tween_position_and_color_together_at_u_half() {
    let mut world = make_world(0.05);
    let tween = Tween::new(0.1, Easing::Linear)
        .with_channel(TweenChannel::PositionX {
            from: 0.0,
            to: 10.0,
        })
        .with_channel(TweenChannel::ColorA { from: 0, to: 200 });
    let entity = spawn_fading_sprite(&mut world, tween);

    tween_tick_system(&mut world);

    let t = world.get::<Transform>(entity).unwrap();
    assert!((t.position.x - 5.0).abs() < 1e-5);
    let s = world.get::<Sprite>(entity).unwrap();
    assert_eq!(s.color[3], 100);
}

#[test]
fn tween_complete_carries_tag() {
    let mut world = make_world(0.2);
    let tween = Tween::new(0.1, Easing::Linear)
        .with_channel(TweenChannel::ColorA { from: 0, to: 255 })
        .with_tag("state_exit");
    let entity = spawn_fading_sprite(&mut world, tween);

    tween_tick_system(&mut world);
    flush(&mut world);

    let events = world.get_resource::<EventQueue<TweenComplete>>().unwrap();
    let found = events
        .iter()
        .find(|e| e.entity == entity)
        .expect("event emitted");
    assert_eq!(found.tag.as_deref(), Some("state_exit"));
}

#[test]
fn tween_without_target_components_is_noop() {
    let mut world = make_world(0.2);
    let entity = world.spawn();
    world.insert(
        entity,
        Tween::new(0.1, Easing::Linear)
            .with_channel(TweenChannel::PositionX { from: 0.0, to: 5.0 }),
    );

    tween_tick_system(&mut world);
    flush(&mut world);

    assert!(world.get::<Tween>(entity).is_none(), "tween still removes");
}

#[test]
fn scene_tween_spawns_component_through_command_buffer() {
    use crate::asset_loader::spawn_scene;
    use tungsten_core::assets::{
        SceneData, SceneEntry, SceneTransform, SceneTween, SceneTweenChannel, SceneTweenRepeat,
    };

    let mut world = World::new();
    world.insert_resource(CommandBuffer::new());
    world.insert_resource(EventQueue::<TweenComplete>::new());

    let data = SceneData {
        entities: vec![SceneEntry {
            transform: SceneTransform {
                position: [0.0, 0.0],
                rotation: 0.0,
                scale: [1.0, 1.0],
            },
            sprite: None,
            visible: true,
            tag: None,
            tweens: vec![SceneTween {
                duration: 0.5,
                easing: Easing::CubicOut,
                repeat: SceneTweenRepeat::Once,
                tag: Some("scene_fade_in".to_string()),
                channels: vec![SceneTweenChannel::ColorA { from: 0, to: 255 }],
            }],
        }],
    };

    spawn_scene(&mut world, &data, "gameplay");
    let buf = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush(buf);
    world.insert_resource(CommandBuffer::new());

    let tweens: Vec<_> = world.query::<Tween>().collect();
    assert_eq!(tweens.len(), 1);
    let (_, tween) = tweens[0];
    assert_eq!(tween.duration, 0.5);
    assert_eq!(tween.easing, Easing::CubicOut);
    assert_eq!(tween.on_complete_tag.as_deref(), Some("scene_fade_in"));
    assert_eq!(world.query2_entities::<Transform, Tween>().len(), 1);
}
