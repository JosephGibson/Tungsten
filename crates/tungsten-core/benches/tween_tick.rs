//! Tween tick: 5k entities × two channels. Inlined to keep `tungsten-core` bench self-contained.

use criterion::{Criterion, criterion_group, criterion_main};
use glam::Vec2;
use std::hint::black_box;

use tungsten_core::{
    Easing, Sprite, SpriteAssetId, Transform, Tween, TweenChannel, TweenRepeat, Visibility, World,
    lerp_f32, lerp_u8,
};

fn build_world(n: usize) -> (World, Vec<tungsten_core::Entity>) {
    let mut world = World::new();
    let mut entities = Vec::with_capacity(n);
    for i in 0..n {
        let e = world.spawn();
        world.insert(
            e,
            Transform::from_position(Vec2::new((i % 100) as f32, (i / 100) as f32)),
        );
        world.insert(e, Sprite::new(SpriteAssetId::new(0)));
        world.insert(e, Visibility::default());
        let duration = 1.0 + ((i % 64) as f32) * (1.0 / 64.0);
        // Its channels through the builders, which alone write them.
        let mut tween = Tween::new(duration, Easing::CubicInOut)
            .with_channel(TweenChannel::PositionX {
                from: 0.0,
                to: 200.0,
            })
            .with_channel(TweenChannel::ColorA { from: 0, to: 255 })
            .with_repeat(TweenRepeat::Loop);
        tween.elapsed = (i % 32) as f32 * (duration / 32.0);
        world.insert(e, tween);
        entities.push(e);
    }
    (world, entities)
}

fn tick_inline(world: &mut World, entities: &[tungsten_core::Entity], dt: f32) {
    for &entity in entities {
        let (duration, elapsed, easing, channels) = match world.get::<Tween>(entity) {
            Some(t) => (t.duration, t.elapsed, t.easing, t.channels().to_vec()),
            None => continue,
        };
        let new_elapsed = (elapsed + dt).clamp(0.0, duration);
        let u = (new_elapsed / duration).clamp(0.0, 1.0);
        let k = easing.apply(u);

        for eased in &channels {
            match &eased.channel {
                TweenChannel::PositionX { from, to } => {
                    if let Some(t) = world.get_mut::<Transform>(entity) {
                        t.position.x = lerp_f32(*from, *to, k);
                    }
                }
                TweenChannel::ColorA { from, to } => {
                    if let Some(s) = world.get_mut::<Sprite>(entity) {
                        s.color[3] = lerp_u8(*from, *to, k);
                    }
                }
                _ => {}
            }
        }

        if let Some(t) = world.get_mut::<Tween>(entity) {
            // Loop: rewind on boundary so population stays constant across iterations.
            t.elapsed = if new_elapsed >= duration {
                0.0
            } else {
                new_elapsed
            };
        }
    }
}

fn bench_tween_tick_5k(c: &mut Criterion) {
    const N: usize = 5_000;
    let (mut world, entities) = build_world(N);
    let dt = 1.0_f32 / 60.0;

    c.bench_function("tween_tick_5k", |b| {
        b.iter(|| {
            tick_inline(&mut world, &entities, black_box(dt));
            black_box(entities.len());
        });
    });
}

criterion_group!(benches, bench_tween_tick_5k);
criterion_main!(benches);
