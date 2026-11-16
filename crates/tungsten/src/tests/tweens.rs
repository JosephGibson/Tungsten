use tungsten_core::{
    CommandBuffer, Easing, Entity, EventQueue, Sprite, SpriteAssetId, Time, Transform, Tween,
    TweenChannel, TweenComplete, TweenDirection, TweenRepeat, Visibility, With, World,
};

use crate::tweens::tween_tick_system;

fn make_world(dt: f32) -> World {
    let mut world = World::new();
    let mut time = Time::new();
    time.advance_frame(dt);
    world.insert_resource(time);
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
    world.insert(entity, Sprite::new(SpriteAssetId::new(0)));
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
fn a_loop_carries_its_overshoot_so_its_period_holds() {
    // 0.375 and 1.0 are exact in binary: eight frames are three whole cycles.
    let mut world = make_world(0.375);
    let tween = Tween::new(1.0, Easing::Linear)
        .with_channel(TweenChannel::ColorA { from: 0, to: 255 })
        .with_repeat(TweenRepeat::Loop);
    let entity = spawn_fading_sprite(&mut world, tween);
    let mut seen = Vec::new();
    for _ in 0..8 {
        tween_tick_system(&mut world);
        seen.push(world.get::<Tween>(entity).unwrap().elapsed);
    }
    assert_eq!(seen, [0.375, 0.75, 0.125, 0.5, 0.875, 0.25, 0.625, 0.0]);
}

#[test]
fn a_ping_pong_reflects_its_overshoot_so_its_period_holds() {
    // 0.375 and 1.0 are exact in binary: eight frames are a period and a half.
    let mut world = make_world(0.375);
    let tween = Tween::new(1.0, Easing::Linear)
        .with_channel(TweenChannel::ColorA { from: 0, to: 255 })
        .with_repeat(TweenRepeat::PingPong);
    let entity = spawn_fading_sprite(&mut world, tween);
    let mut seen = Vec::new();
    for _ in 0..8 {
        tween_tick_system(&mut world);
        let state = world.get::<Tween>(entity).unwrap();
        seen.push((state.elapsed, state.direction == TweenDirection::Forward));
    }
    assert_eq!(
        seen,
        [
            (0.375, true),
            (0.75, true),
            (0.875, false),
            (0.5, false),
            (0.125, false),
            (0.25, true),
            (0.625, true),
            (1.0, false),
        ]
    );
}

#[test]
fn times_completes_on_the_frame_its_cycles_add_up() {
    let mut world = make_world(0.375);
    let tween = Tween::new(1.0, Easing::Linear)
        .with_channel(TweenChannel::ColorA { from: 0, to: 255 })
        .with_repeat(TweenRepeat::Times(3));
    let entity = spawn_fading_sprite(&mut world, tween);
    for frame in 1..=8 {
        tween_tick_system(&mut world);
        let done = world.get::<Tween>(entity).unwrap().pending_remove;
        assert_eq!(done, frame == 8, "frame {frame}");
    }
}

#[test]
fn a_backward_loop_restarts_from_its_end() {
    let mut world = make_world(0.375);
    let tween = Tween::new(1.0, Easing::Linear)
        .with_channel(TweenChannel::ColorA { from: 0, to: 255 })
        .with_repeat(TweenRepeat::Loop);
    let entity = spawn_fading_sprite(&mut world, tween);
    let state = world.get_mut::<Tween>(entity).unwrap();
    state.direction = TweenDirection::Backward;
    state.elapsed = 1.0;
    let mut seen = Vec::new();
    for _ in 0..3 {
        tween_tick_system(&mut world);
        seen.push(world.get::<Tween>(entity).unwrap().elapsed);
    }
    assert_eq!(seen, [0.625, 0.25, 0.875]);
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
    assert!(
        (state.elapsed - 0.08).abs() < 1e-6,
        "the 0.02 s past the end reflected back: {}",
        state.elapsed
    );
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
            components: std::collections::BTreeMap::new(),
        }],
    };

    spawn_scene(&mut world, &data, "gameplay");
    let buf = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush(buf);
    world.insert_resource(CommandBuffer::new());

    let tweens: Vec<_> = world.query::<(Entity, &Tween)>().collect();
    assert_eq!(tweens.len(), 1);
    let (_, tween) = tweens[0];
    assert_eq!(tween.duration, 0.5);
    assert_eq!(tween.easing, Easing::CubicOut);
    assert_eq!(tween.on_complete_tag.as_deref(), Some("scene_fade_in"));
    assert_eq!(
        world
            .query_filtered::<Entity, (With<Transform>, With<Tween>)>()
            .count(),
        1
    );
}

#[test]
fn replacement_tween_queued_before_completion_survives_flush() {
    // B4: a system queues a replacement through the `CommandBuffer` before the
    // tween stage completes the old tween; the old tween's deferred removal
    // must not delete the replacement at flush.
    let dt = 0.05;
    let mut world = make_world(dt);
    let entity = spawn_fading_sprite(
        &mut world,
        Tween::new(dt, Easing::Linear).with_channel(TweenChannel::ColorA { from: 0, to: 255 }),
    );
    let replacement =
        Tween::new(1.0, Easing::Linear).with_channel(TweenChannel::ColorA { from: 255, to: 0 });
    world
        .get_resource_mut::<CommandBuffer>()
        .unwrap()
        .insert(entity, replacement);

    tween_tick_system(&mut world);
    flush(&mut world);

    let tween = world
        .get::<Tween>(entity)
        .expect("the replacement survives the flush");
    assert_eq!(tween.duration, 1.0);
    assert!(!tween.pending_remove);
}

#[test]
fn a_duration_written_past_new_is_sanitised() {
    // `Tween`'s fields are public, so `duration` can skip `Tween::new`'s
    // clamp: a negative or NaN one reached `f32::clamp`'s assert every
    // frame, and 0 wrote 0 / 0 into the transform.
    for duration in [-1.0, f32::NAN, 0.0, f32::INFINITY] {
        let mut world = make_world(0.016);
        let mut tween =
            Tween::new(1.0, Easing::Linear).with_channel(TweenChannel::ColorA { from: 0, to: 255 });
        tween.duration = duration;
        let entity = spawn_fading_sprite(&mut world, tween);
        tween_tick_system(&mut world);
        flush(&mut world);
        let sprite = world.get::<Sprite>(entity).expect("sprite survives");
        assert_eq!(sprite.color[3], 255, "{duration}: finishes at once");
        assert!(world.get::<Tween>(entity).is_none(), "{duration}: removed");
    }
}

#[test]
fn a_light_intensity_tween_writes_the_light() {
    let mut world = make_world(0.05);
    let lamp = world.spawn();
    world.insert(lamp, Transform::default());
    world.insert(lamp, tungsten_core::Light::point(glam::Vec3::ONE, 64.0));
    world.insert(
        lamp,
        Tween::new(0.1, Easing::Linear)
            .with_channel(TweenChannel::LightIntensity { from: 2.0, to: 0.0 }),
    );
    // A tween on an entity without a light writes nothing and never panics.
    let dark = world.spawn();
    world.insert(dark, Transform::default());
    world.insert(
        dark,
        Tween::new(0.1, Easing::Linear)
            .with_channel(TweenChannel::LightIntensity { from: 2.0, to: 0.0 }),
    );
    tween_tick_system(&mut world);
    let intensity = world.get::<tungsten_core::Light>(lamp).unwrap().intensity;
    assert!((intensity - 1.0).abs() < 1e-5, "{intensity}");
    tween_tick_system(&mut world);
    assert_eq!(
        world.get::<tungsten_core::Light>(lamp).unwrap().intensity,
        0.0
    );
    // An overshooting easing stops at 0.
    world.insert(
        lamp,
        Tween::new(0.1, Easing::BackIn)
            .with_channel(TweenChannel::LightIntensity { from: 0.0, to: 1.0 }),
    );
    tween_tick_system(&mut world);
    assert!(world.get::<tungsten_core::Light>(lamp).unwrap().intensity >= 0.0);
}

#[test]
fn each_channel_eases_on_its_own() {
    // A 1 s tween, Linear, its scale eased `CubicOut` from 1 to 2 and its
    // alpha from 255 to 0 on the tween's own.
    let mut world = make_world(0.5);
    let tween = Tween::new(1.0, Easing::Linear)
        .with_channel_eased(
            TweenChannel::ScaleX { from: 1.0, to: 2.0 },
            Easing::CubicOut,
        )
        .with_channel(TweenChannel::ColorA { from: 255, to: 0 });
    let entity = spawn_fading_sprite(&mut world, tween);
    let read = |world: &World| {
        (
            world.get::<Transform>(entity).unwrap().scale.x,
            world.get::<Sprite>(entity).unwrap().color[3],
        )
    };
    tween_tick_system(&mut world);
    assert_eq!(
        read(&world),
        (1.875, 128),
        "at 0.5 s: 1 + 0.875, half of 255"
    );
    tween_tick_system(&mut world);
    assert_eq!(read(&world), (2.0, 0), "at 1 s each at its end");
    // A ping-pong's way back retraces each channel's own curve.
    let mut world = make_world(0.5);
    let tween = Tween::new(1.0, Easing::Linear)
        .with_channel_eased(
            TweenChannel::ScaleX { from: 1.0, to: 2.0 },
            Easing::CubicOut,
        )
        .with_channel(TweenChannel::ColorA { from: 255, to: 0 })
        .with_repeat(TweenRepeat::PingPong);
    let entity = spawn_fading_sprite(&mut world, tween);
    let read = |world: &World| {
        (
            world.get::<Transform>(entity).unwrap().scale.x,
            world.get::<Sprite>(entity).unwrap().color[3],
        )
    };
    let mut seen = Vec::new();
    for _ in 0..4 {
        tween_tick_system(&mut world);
        seen.push(read(&world));
    }
    assert_eq!(seen, [(1.875, 128), (2.0, 0), (1.875, 128), (1.0, 255)]);
    // No eased channel: every channel by the tween's easing, as before.
    let mut world = make_world(0.5);
    let tween = Tween::new(1.0, Easing::CubicOut)
        .with_channel(TweenChannel::ScaleX { from: 1.0, to: 2.0 })
        .with_channel(TweenChannel::ColorA { from: 0, to: 200 });
    let entity = spawn_fading_sprite(&mut world, tween);
    tween_tick_system(&mut world);
    assert_eq!(world.get::<Transform>(entity).unwrap().scale.x, 1.875);
    assert_eq!(world.get::<Sprite>(entity).unwrap().color[3], 175);
}

#[test]
fn a_tween_despawns_its_entity_at_its_end() {
    let completes = |world: &World, entity: Entity| {
        world
            .resource::<EventQueue<TweenComplete>>()
            .iter()
            .filter(|done| done.entity == entity)
            .count()
    };
    let fade =
        || Tween::new(0.1, Easing::Linear).with_channel(TweenChannel::ColorA { from: 255, to: 0 });
    // Once: its end sends its `TweenComplete` and the flush ends the entity.
    let mut world = make_world(0.05);
    let entity = spawn_fading_sprite(&mut world, fade().with_despawn_at_end());
    tween_tick_system(&mut world);
    flush(&mut world);
    assert!(world.is_alive(entity), "halfway");
    tween_tick_system(&mut world);
    assert_eq!(completes(&world, entity), 1);
    assert!(world.is_alive(entity), "until the flush");
    flush(&mut world);
    assert!(!world.is_alive(entity), "gone at the flush");
    // Times(2): at its second end.
    let mut world = make_world(0.05);
    let tween = fade()
        .with_repeat(TweenRepeat::Times(2))
        .with_despawn_at_end();
    let entity = spawn_fading_sprite(&mut world, tween);
    for _ in 0..2 {
        tween_tick_system(&mut world);
    }
    flush(&mut world);
    assert!(world.is_alive(entity), "after its first end");
    for _ in 0..2 {
        tween_tick_system(&mut world);
    }
    flush(&mut world);
    assert_eq!(completes(&world, entity), 1);
    assert!(!world.is_alive(entity), "at its second end");
    // A replacement inserted earlier in the same flush keeps the entity.
    let mut world = make_world(0.1);
    let entity = spawn_fading_sprite(&mut world, fade().with_despawn_at_end());
    world
        .resource_mut::<CommandBuffer>()
        .insert(entity, Tween::new(1.0, Easing::Linear));
    tween_tick_system(&mut world);
    flush(&mut world);
    assert!(world.is_alive(entity), "the replacement keeps it");
    assert_eq!(world.get::<Tween>(entity).unwrap().duration, 1.0);
    // Without it, the tween alone goes, as before.
    let mut world = make_world(0.1);
    let entity = spawn_fading_sprite(&mut world, fade());
    tween_tick_system(&mut world);
    flush(&mut world);
    assert!(world.is_alive(entity));
    assert!(world.get::<Tween>(entity).is_none());
}
