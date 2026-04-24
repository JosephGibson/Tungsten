//! CPU-only render micro-benches; no wgpu device or display.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tungsten_core::assets::{pack_shelf, FilterMode, PackInput, TextureHandle};
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
                batches[tex_idx].instances.push(SpriteInstance::whole(
                    [i as f32, (i / TEXTURES) as f32],
                    [16.0, 16.0],
                    0.0,
                    [255; 4],
                ));
            }

            black_box(batches);
        });
    });
}

/// Deterministic dependency-free RNG for fixed atlas-pack input.
fn xorshift64(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

fn bench_atlas_pack_startup_200(c: &mut Criterion) {
    const N: usize = 200;
    const MAX_DIM: u32 = 8192;
    const PADDING: u32 = 1;

    let mut rng: u64 = 0xA71A5;
    let sizes: Vec<(u32, u32)> = (0..N)
        .map(|_| {
            let w = 8 + (xorshift64(&mut rng) as u32 % 121);
            let h = 8 + (xorshift64(&mut rng) as u32 % 121);
            (w, h)
        })
        .collect();
    let ids: Vec<String> = (0..N).map(|i| format!("s{i:03}")).collect();

    c.bench_function("atlas_pack_startup_200", |b| {
        b.iter(|| {
            let inputs: Vec<PackInput<'_>> = ids
                .iter()
                .zip(sizes.iter())
                .map(|(id, &(w, h))| PackInput {
                    id: id.as_str(),
                    width: w,
                    height: h,
                })
                .collect();
            let result = pack_shelf(&inputs, MAX_DIM, PADDING);
            black_box(result);
        });
    });
}

criterion_group!(
    benches,
    bench_sprite_extract_batch_build_2k,
    bench_atlas_pack_startup_200
);
criterion_main!(benches);
