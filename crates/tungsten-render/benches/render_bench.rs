//! Render-side micro-benchmarks (M12).
//!
//! CPU-only: measures cost of building render data structures.
//! No wgpu device is created; no display or GPU required.
//!
//! Scenarios:
//!   - sprite_extract_batch_build_2k — build SpriteBatch vec for 2k sprites / 10 textures

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tungsten_core::assets::{FilterMode, TextureHandle};
use tungsten_render::{SpriteBatch, SpriteInstance};

fn bench_sprite_extract_batch_build_2k(c: &mut Criterion) {
    const N: usize = 2_000;
    const TEXTURES: usize = 10;

    c.bench_function("sprite_extract_batch_build_2k", |b| {
        b.iter(|| {
            let mut batches: Vec<SpriteBatch> = (0..TEXTURES)
                .map(|i| SpriteBatch {
                    texture: TextureHandle(i as u32),
                    filter: FilterMode::Nearest,
                    instances: Vec::new(),
                })
                .collect();

            for i in 0..N {
                let tex_idx = i % TEXTURES;
                batches[tex_idx].instances.push(SpriteInstance {
                    position: [i as f32, (i / TEXTURES) as f32],
                    size: [16.0, 16.0],
                    rotation: 0.0,
                    color: [255; 4],
                });
            }

            black_box(batches);
        });
    });
}

criterion_group!(benches, bench_sprite_extract_batch_build_2k);
criterion_main!(benches);
