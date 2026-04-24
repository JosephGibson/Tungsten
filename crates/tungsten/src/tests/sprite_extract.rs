use super::*;
use glam::Vec2;
use std::path::PathBuf;
use tungsten_core::assets::{TextureHandle, UvRect};
use tungsten_core::{AssetRegistry, Sprite, Transform, Visibility, World};

fn register_sprite(world: &mut World, id: &str, filter: FilterMode) {
    let registry = world
        .get_resource_mut::<AssetRegistry>()
        .expect("AssetRegistry resource missing");
    registry.register_sprite(
        id.to_string(),
        filter,
        16,
        16,
        PathBuf::from(format!("test/{id}.png")),
        TextureHandle(0),
        UvRect::FULL,
    );
}

fn world_with_registry() -> World {
    let mut world = World::new();
    world.insert_resource(AssetRegistry::new());
    world
}

#[test]
fn missing_visibility_emits_nothing() {
    let mut world = world_with_registry();
    register_sprite(&mut world, "quad", FilterMode::Nearest);

    let e = world.spawn();
    world.insert(e, Transform::default());
    world.insert(e, Sprite::new("quad"));

    let batches = extract_sprites_default(&world);
    let total: usize = batches.iter().map(|b| b.instances.len()).sum();
    assert_eq!(total, 0);
}

#[test]
fn invisible_entity_emits_nothing() {
    let mut world = world_with_registry();
    register_sprite(&mut world, "quad", FilterMode::Nearest);

    let e = world.spawn();
    world.insert(e, Transform::default());
    world.insert(e, Sprite::new("quad"));
    world.insert(e, Visibility { visible: false });

    let batches = extract_sprites_default(&world);
    let total: usize = batches.iter().map(|b| b.instances.len()).sum();
    assert_eq!(total, 0);
}

#[test]
fn missing_asset_id_emits_nothing() {
    let mut world = world_with_registry();

    let e = world.spawn();
    world.insert(e, Transform::default());
    world.insert(e, Sprite::new("ghost"));
    world.insert(e, Visibility::default());

    let batches = extract_sprites_default(&world);
    let total: usize = batches.iter().map(|b| b.instances.len()).sum();
    assert_eq!(total, 0);
}

#[test]
fn transform_scale_applies_to_instance_size() {
    let mut world = world_with_registry();
    register_sprite(&mut world, "quad", FilterMode::Nearest);

    let e = world.spawn();
    world.insert(
        e,
        Transform {
            position: Vec2::new(5.0, 7.0),
            rotation: 0.5,
            scale: Vec2::new(2.0, 3.0),
        },
    );
    world.insert(e, Sprite::new("quad"));
    world.insert(e, Visibility::default());

    let batches = extract_sprites_default(&world);
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].instances.len(), 1);
    let inst = &batches[0].instances[0];
    assert_eq!(inst.position, [5.0, 7.0]);
    assert_eq!(inst.size, [32.0, 48.0]);
    assert_eq!(inst.rotation, 0.5);
    assert_eq!(inst.color, [255; 4]);
    assert_eq!(inst.uv_min, [0.0, 0.0]);
    assert_eq!(inst.uv_size, [1.0, 1.0]);
}

#[test]
fn sprite_color_reaches_instance() {
    let mut world = world_with_registry();
    register_sprite(&mut world, "quad", FilterMode::Nearest);

    let e = world.spawn();
    world.insert(e, Transform::default());
    world.insert(
        e,
        Sprite {
            asset_id: "quad".into(),
            color: [10, 20, 30, 255],
            z_order: 0,
            material_id: None,
        },
    );
    world.insert(e, Visibility::default());

    let batches = extract_sprites_default(&world);
    assert_eq!(batches[0].instances[0].color, [10, 20, 30, 255]);
}

#[test]
fn z_order_groups_do_not_merge_across_a_lower_z_entry() {
    let mut world = world_with_registry();
    register_sprite(&mut world, "quad", FilterMode::Nearest);

    // Batch map resets between z-order runs.
    for z in [-1, 0, 1] {
        let e = world.spawn();
        world.insert(e, Transform::default());
        world.insert(
            e,
            Sprite {
                asset_id: "quad".into(),
                color: [255; 4],
                z_order: z,
                material_id: None,
            },
        );
        world.insert(e, Visibility::default());
    }

    let batches = extract_sprites_default(&world);
    assert_eq!(batches.len(), 3);
    assert!(batches.iter().all(|b| b.instances.len() == 1));
}

#[test]
fn same_z_same_texture_collapses_to_one_batch() {
    let mut world = world_with_registry();
    register_sprite(&mut world, "quad", FilterMode::Nearest);

    for _ in 0..4 {
        let e = world.spawn();
        world.insert(e, Transform::default());
        world.insert(e, Sprite::new("quad"));
        world.insert(e, Visibility::default());
    }

    let batches = extract_sprites_default(&world);
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].instances.len(), 4);
}

#[test]
fn missing_asset_registry_returns_empty() {
    let world = World::new();
    let batches = extract_sprites_default(&world);
    assert!(batches.is_empty());
}

#[test]
fn same_z_order_breaks_ties_by_entity_id() {
    // D-NNN (M25): `GpuDepth` needs a deterministic tie-break for same-z
    // entries so depth-test output reproduces `CpuStable` painter order.
    // Extract must sort primarily by `z_order` and then by `Entity::id()`.
    let mut world = world_with_registry();
    register_sprite(&mut world, "a", FilterMode::Nearest);
    register_sprite(&mut world, "b", FilterMode::Nearest);

    let e0 = world.spawn();
    world.insert(e0, Transform::default());
    world.insert(
        e0,
        Sprite {
            asset_id: "a".into(),
            color: [1, 0, 0, 255],
            z_order: 0,
            material_id: None,
        },
    );
    world.insert(e0, Visibility::default());

    let e1 = world.spawn();
    world.insert(e1, Transform::default());
    world.insert(
        e1,
        Sprite {
            asset_id: "b".into(),
            color: [0, 2, 0, 255],
            z_order: 0,
            material_id: None,
        },
    );
    world.insert(e1, Visibility::default());

    let batches = extract_sprites_default(&world);
    // Same atlas + filter -> batched together in painter order. The first
    // instance drawn must carry `e0`'s color because `e0.id() < e1.id()`.
    assert_eq!(batches.len(), 1);
    let instances = &batches[0].instances;
    assert_eq!(instances.len(), 2);
    assert_eq!(instances[0].color, [1, 0, 0, 255]);
    assert_eq!(instances[1].color, [0, 2, 0, 255]);
}

#[test]
fn z_norm_decreases_along_painter_order_for_less_equal_depth_test() {
    // Under `depth_compare = LessEqual` with a 1.0 clear, later-painted
    // fragments must have SMALLER z to pass the test and overwrite earlier
    // overlaps. Extract must emit `z_norm` so painter order (sorted by
    // `(z_order, entity_id)`) produces strictly-decreasing z_norm values,
    // and the last drawn instance reaches 0.0.
    let mut world = world_with_registry();
    register_sprite(&mut world, "quad", FilterMode::Nearest);

    // Spawn three sprites at z = 2, 0, 1 -- order after sort: 0, 1, 2.
    for z in [2, 0, 1] {
        let e = world.spawn();
        world.insert(e, Transform::default());
        world.insert(
            e,
            Sprite {
                asset_id: "quad".into(),
                color: [255; 4],
                z_order: z,
                material_id: None,
            },
        );
        world.insert(e, Visibility::default());
    }

    let batches = extract_sprites_default(&world);
    let zs: Vec<f32> = batches
        .iter()
        .flat_map(|b| b.instances.iter().map(|i| i.z_norm))
        .collect();
    assert_eq!(zs.len(), 3);
    // Later-drawn fragments must have strictly smaller z so `LessEqual`
    // accepts them over existing pixels from earlier draws.
    for w in zs.windows(2) {
        assert!(w[0] > w[1], "z_norm not strictly decreasing: {w:?}");
    }
    // Last painted wins → z_norm == 0.0.
    assert!((zs[2] - 0.0).abs() < 1e-6);
    // First painted is furthest back but stays strictly < 1.0 so it never
    // gets clipped by wgpu's [0, 1] NDC-z range.
    assert!(zs[0] < 1.0);
    assert!(zs[0] > 0.0);
}
