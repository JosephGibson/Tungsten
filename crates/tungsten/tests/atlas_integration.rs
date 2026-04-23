//! Atlas extract seam: shared TextureHandle batches; distinct atlases split.

use std::path::PathBuf;

use glam::Vec2;
use tungsten::extract_sprites_default;
use tungsten_core::assets::{TextureHandle, UvRect};
use tungsten_core::{AssetRegistry, FilterMode, Sprite, Transform, Visibility, World};

fn world_with_registry() -> World {
    let mut world = World::new();
    world.insert_resource(AssetRegistry::new());
    world
}

fn register(world: &mut World, id: &str, filter: FilterMode, atlas: TextureHandle, uv: UvRect) {
    let registry = world
        .get_resource_mut::<AssetRegistry>()
        .expect("AssetRegistry resource missing");
    registry.register_sprite(
        id.to_string(),
        filter,
        16,
        16,
        PathBuf::from(format!("test/{id}.png")),
        atlas,
        uv,
    );
}

fn spawn(world: &mut World, id: &str, position: Vec2) {
    let e = world.spawn();
    world.insert(
        e,
        Transform {
            position,
            rotation: 0.0,
            scale: Vec2::splat(1.0),
        },
    );
    world.insert(
        e,
        Sprite {
            asset_id: id.to_string(),
            color: [255; 4],
            z_order: 0,
        },
    );
    world.insert(e, Visibility::default());
}

#[test]
fn two_sprites_sharing_atlas_collapse_to_one_batch() {
    let mut world = world_with_registry();

    let uv_a = UvRect {
        min: [0.0, 0.0],
        max: [0.5, 1.0],
    };
    let uv_b = UvRect {
        min: [0.5, 0.0],
        max: [1.0, 1.0],
    };
    register(
        &mut world,
        "left",
        FilterMode::Nearest,
        TextureHandle(0),
        uv_a,
    );
    register(
        &mut world,
        "right",
        FilterMode::Nearest,
        TextureHandle(0),
        uv_b,
    );

    spawn(&mut world, "left", Vec2::new(0.0, 0.0));
    spawn(&mut world, "right", Vec2::new(32.0, 0.0));

    let batches = extract_sprites_default(&world);

    assert_eq!(batches.len(), 1, "shared atlas must produce a single batch");
    assert_eq!(batches[0].instances.len(), 2);
    assert_eq!(batches[0].texture, TextureHandle(0));
    assert_eq!(batches[0].filter, FilterMode::Nearest);

    let uv_mins: Vec<[f32; 2]> = batches[0].instances.iter().map(|i| i.uv_min).collect();
    assert_ne!(
        uv_mins[0], uv_mins[1],
        "each instance keeps its own uv_min slice of the page"
    );
}

#[test]
fn three_sprites_across_two_atlases_produce_two_batches() {
    let mut world = world_with_registry();

    // TextureHandle alone splits batches here.
    let uv_a0 = UvRect {
        min: [0.0, 0.0],
        max: [0.5, 1.0],
    };
    let uv_a1 = UvRect {
        min: [0.5, 0.0],
        max: [1.0, 1.0],
    };
    let uv_b = UvRect::FULL;
    register(
        &mut world,
        "a0",
        FilterMode::Nearest,
        TextureHandle(1),
        uv_a0,
    );
    register(
        &mut world,
        "a1",
        FilterMode::Nearest,
        TextureHandle(1),
        uv_a1,
    );
    register(
        &mut world,
        "b0",
        FilterMode::Nearest,
        TextureHandle(2),
        uv_b,
    );

    spawn(&mut world, "a0", Vec2::new(0.0, 0.0));
    spawn(&mut world, "a1", Vec2::new(16.0, 0.0));
    spawn(&mut world, "b0", Vec2::new(32.0, 0.0));

    let batches = extract_sprites_default(&world);

    assert_eq!(batches.len(), 2, "two distinct atlases → two batches");
    let mut sizes: Vec<usize> = batches.iter().map(|b| b.instances.len()).collect();
    sizes.sort_unstable();
    assert_eq!(sizes, vec![1, 2]);

    for batch in &batches {
        match batch.texture {
            TextureHandle(1) => assert_eq!(batch.instances.len(), 2),
            TextureHandle(2) => assert_eq!(batch.instances.len(), 1),
            other => panic!("unexpected texture handle {other:?}"),
        }
    }
}
