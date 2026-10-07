use super::*;
use glam::Vec2;
use std::path::PathBuf;
use tungsten_core::assets::{TextureHandle, UvRect};
use tungsten_core::{
    AssetRegistry, Entity, Pcg32, Sprite, SpriteAssetId, Transform, Visibility, World,
};

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
        None,
        None,
        None,
    );
}

fn world_with_registry() -> World {
    let mut world = World::new();
    world.insert_resource(AssetRegistry::new());
    world
}

/// The world registry's ID for `name`, interned if new.
fn intern(world: &mut World, name: &str) -> SpriteAssetId {
    world
        .get_resource_mut::<AssetRegistry>()
        .expect("AssetRegistry resource missing")
        .intern_sprite(name)
}

#[test]
fn missing_visibility_emits_nothing() {
    let mut world = world_with_registry();
    register_sprite(&mut world, "quad", FilterMode::Nearest);

    let e = world.spawn();
    world.insert(e, Transform::default());
    let sprite = Sprite::new(intern(&mut world, "quad"));
    world.insert(e, sprite);

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
    let sprite = Sprite::new(intern(&mut world, "quad"));
    world.insert(e, sprite);
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
    let sprite = Sprite::new(intern(&mut world, "ghost"));
    world.insert(e, sprite);
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
    let sprite = Sprite::new(intern(&mut world, "quad"));
    world.insert(e, sprite);
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
    let quad = intern(&mut world, "quad");

    let e = world.spawn();
    world.insert(e, Transform::default());
    world.insert(
        e,
        Sprite {
            asset_id: quad,
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
    let quad = intern(&mut world, "quad");

    // Batch map resets between z-order runs.
    for z in [-1, 0, 1] {
        let e = world.spawn();
        world.insert(e, Transform::default());
        world.insert(
            e,
            Sprite {
                asset_id: quad,
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
        let sprite = Sprite::new(intern(&mut world, "quad"));
        world.insert(e, sprite);
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
    let (a, b) = (intern(&mut world, "a"), intern(&mut world, "b"));

    let e0 = world.spawn();
    world.insert(e0, Transform::default());
    world.insert(
        e0,
        Sprite {
            asset_id: a,
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
            asset_id: b,
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
    let quad = intern(&mut world, "quad");

    // Spawn three sprites at z = 2, 0, 1 -- order after sort: 0, 1, 2.
    for z in [2, 0, 1] {
        let e = world.spawn();
        world.insert(e, Transform::default());
        world.insert(
            e,
            Sprite {
                asset_id: quad,
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

fn register_lit_sprite(world: &mut World, id: &str) {
    let registry = world
        .get_resource_mut::<AssetRegistry>()
        .expect("AssetRegistry resource missing");
    registry.register_sprite(
        id.to_string(),
        FilterMode::Nearest,
        16,
        16,
        PathBuf::from(format!("test/{id}.png")),
        TextureHandle(7),
        UvRect::FULL,
        Some(PathBuf::from(format!("test/{id}_n.png"))),
        Some(PathBuf::from(format!("test/{id}_e.png"))),
        Some(TextureHandle(7)),
    );
}

#[test]
fn lit_batch_routed_when_lit_atlas_present() {
    let mut world = world_with_registry();
    register_lit_sprite(&mut world, "lit_quad");

    let e = world.spawn();
    world.insert(e, Transform::default());
    let sprite = Sprite::new(intern(&mut world, "lit_quad"));
    world.insert(e, sprite);
    world.insert(e, Visibility::default());

    let batches = extract_sprites_default(&world);
    assert_eq!(batches.len(), 1);
    assert!(batches[0].lit, "lit batch must opt into lit pipeline");
    assert!(
        batches[0].material_id.is_none(),
        "lit batch must not carry a material id"
    );
}

#[test]
fn unlit_path_byte_identical_with_no_aux() {
    let mut world = world_with_registry();
    register_sprite(&mut world, "quad", FilterMode::Nearest);

    let e = world.spawn();
    world.insert(e, Transform::default());
    let sprite = Sprite::new(intern(&mut world, "quad"));
    world.insert(e, sprite);
    world.insert(e, Visibility::default());

    let batches = extract_sprites_default(&world);
    assert_eq!(batches.len(), 1);
    assert!(!batches[0].lit, "unlit sprite must keep lit = false");
}

fn spawn_visible(world: &mut World, id: &str, position: Vec2, z_order: i32) -> Entity {
    let e = world.spawn();
    world.insert(e, Transform::from_position(position));
    let mut sprite = Sprite::new(intern(world, id));
    sprite.z_order = z_order;
    world.insert(e, sprite);
    world.insert(e, Visibility::default());
    e
}

fn instance_positions(batches: &[tungsten_render::SpriteBatch]) -> Vec<[f32; 2]> {
    batches
        .iter()
        .flat_map(|b| b.instances.iter().map(|i| i.position))
        .collect()
}

#[test]
fn parallax_layer_remaps_instance_position() {
    let mut world = world_with_registry();
    register_sprite(&mut world, "quad", FilterMode::Nearest);
    let mut camera = CameraState::new();
    camera.position = Vec2::new(400.0, 200.0);
    world.insert_resource(camera);

    let e = spawn_visible(&mut world, "quad", Vec2::new(10.0, 20.0), 0);
    world.insert(e, ParallaxLayer::uniform(0.25));

    let batches = extract_sprites_default(&world);
    assert_eq!(
        instance_positions(&batches),
        vec![[10.0 + 400.0 * 0.75, 20.0 + 200.0 * 0.75]]
    );
}

#[test]
fn parallax_world_locked_factor_leaves_position_untouched() {
    let mut world = world_with_registry();
    register_sprite(&mut world, "quad", FilterMode::Nearest);
    let mut camera = CameraState::new();
    camera.position = Vec2::new(900.0, -300.0);
    world.insert_resource(camera);

    let e = spawn_visible(&mut world, "quad", Vec2::new(64.0, 32.0), 0);
    world.insert(e, ParallaxLayer::new(Vec2::ONE));

    let batches = extract_sprites_default(&world);
    assert_eq!(instance_positions(&batches), vec![[64.0, 32.0]]);
}

#[test]
fn sprite_without_parallax_is_unchanged_under_camera_motion() {
    // Every instance field, at full `f32` debug precision: a sprite with no
    // `ParallaxLayer` must extract exactly as it did pre-M30 regardless of
    // where the camera sits.
    let build = |camera_position: Vec2| {
        let mut world = world_with_registry();
        register_sprite(&mut world, "quad", FilterMode::Nearest);
        let mut camera = CameraState::new();
        camera.position = camera_position;
        world.insert_resource(camera);
        spawn_visible(&mut world, "quad", Vec2::new(12.0, 34.0), 0);
        let batches = extract_sprites_default(&world);
        batches
            .iter()
            .map(|b| format!("{:?}{:?}{:?}", b.texture, b.filter, b.instances))
            .collect::<Vec<String>>()
    };

    assert_eq!(build(Vec2::ZERO), build(Vec2::new(750.0, -120.0)));
}

#[test]
fn parallax_does_not_change_batch_grouping_or_z_norm() {
    let mut world = world_with_registry();
    register_sprite(&mut world, "quad", FilterMode::Nearest);
    let mut camera = CameraState::new();
    camera.position = Vec2::new(500.0, 500.0);
    world.insert_resource(camera);

    let far = spawn_visible(&mut world, "quad", Vec2::new(0.0, 0.0), -100);
    world.insert(far, ParallaxLayer::uniform(0.1));
    let near = spawn_visible(&mut world, "quad", Vec2::new(0.0, 0.0), -100);
    world.insert(near, ParallaxLayer::uniform(0.9));
    spawn_visible(&mut world, "quad", Vec2::new(0.0, 0.0), 10);

    let batches = extract_sprites_default(&world);
    // Same atlas, filter, material and z-run: the two parallax layers still
    // collapse into one batch, and the z_order 10 sprite opens the next.
    assert_eq!(batches.len(), 2);
    assert_eq!(batches[0].instances.len(), 2);
    assert_eq!(batches[1].instances.len(), 1);

    let z_norms: Vec<f32> = batches
        .iter()
        .flat_map(|b| b.instances.iter().map(|i| i.z_norm))
        .collect();
    assert_eq!(z_norms, vec![2.0 / 3.0, 1.0 / 3.0, 0.0]);
}

#[test]
fn parallax_without_camera_resource_is_identity() {
    let mut world = world_with_registry();
    register_sprite(&mut world, "quad", FilterMode::Nearest);

    let e = spawn_visible(&mut world, "quad", Vec2::new(7.0, 9.0), 0);
    world.insert(e, ParallaxLayer::uniform(0.0));

    let batches = extract_sprites_default(&world);
    assert_eq!(instance_positions(&batches), vec![[7.0, 9.0]]);
}

// Golden output (2026-10): the default extract was rewritten around reusable
// scratch buffers and a compact sort. `reference_extract` is the extract as it
// stood before, kept so the rewrite is checked against it byte for byte.

/// Byte-at-a-time override hash of the pre-rewrite extract.
fn reference_override_key(block: &UniformOverrideBlock) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for byte in block.to_bytes() {
        byte.hash(&mut hasher);
    }
    hasher.finish()
}

fn reference_extract(world: &World) -> Vec<SpriteBatch> {
    type Key = (u32, FilterMode, Option<MaterialAssetId>, Option<u64>, bool);
    let Some(assets) = world.get_resource::<AssetRegistry>() else {
        return Vec::new();
    };
    let camera_position = world
        .get_resource::<CameraState>()
        .map_or(Vec2::ZERO, |camera| camera.position);

    let mut entries: Vec<_> = world
        .query::<(
            Entity,
            &Transform,
            &Sprite,
            &Visibility,
            Option<&UniformOverrideBlock>,
            Option<&ParallaxLayer>,
        )>()
        .filter_map(|(e, t, s, v, override_block, parallax)| {
            if !v.visible {
                return None;
            }
            let asset = assets.sprite(s.asset_id)?;
            Some((e, t, s, asset, override_block, parallax))
        })
        .collect();
    entries.sort_by(|a, b| {
        a.2.z_order
            .cmp(&b.2.z_order)
            .then_with(|| a.0.id().cmp(&b.0.id()))
    });

    let total = entries.len();
    let mut out: Vec<SpriteBatch> = Vec::new();
    let mut current_z: Option<i32> = None;
    let mut per_key: HashMap<Key, usize> = HashMap::new();
    for (idx_in_order, &(_entity, t, s, asset, override_block, parallax)) in
        entries.iter().enumerate()
    {
        if current_z != Some(s.z_order) {
            per_key.clear();
            current_z = Some(s.z_order);
        }
        let override_hash = override_block.map(reference_override_key);
        let lit = asset.lit_atlas.is_some();
        let effective_material = if lit { None } else { s.material_id };
        let key: Key = (
            asset.atlas.0,
            asset.filter,
            effective_material,
            override_hash,
            lit,
        );
        let batch_idx = if let Some(&i) = per_key.get(&key) {
            i
        } else {
            let i = out.len();
            per_key.insert(key, i);
            let mut batch = SpriteBatch::new(asset.atlas, asset.filter);
            batch.material_id = effective_material;
            batch.uniform_overrides = if lit { None } else { override_block.copied() };
            batch.lit = lit;
            out.push(batch);
            i
        };
        let z_norm = if total > 0 {
            (total as f32 - 1.0 - idx_in_order as f32) / total as f32
        } else {
            0.0
        };
        let position = match parallax {
            Some(layer) => {
                parallax_world_position(t.position, layer.scroll_factor, camera_position)
            }
            None => t.position,
        };
        out[batch_idx].instances.push(SpriteInstance {
            position: [position.x, position.y],
            size: [
                asset.width as f32 * t.scale.x,
                asset.height as f32 * t.scale.y,
            ],
            rotation: t.rotation,
            color: s.color,
            uv_min: asset.uv.min,
            uv_size: [
                asset.uv.max[0] - asset.uv.min[0],
                asset.uv.max[1] - asset.uv.min[1],
            ],
            z_norm,
            _pad: 0.0,
        });
    }
    out
}

fn register_on_page(
    world: &mut World,
    id: &str,
    filter: FilterMode,
    page: u32,
    size: (u32, u32),
    uv: UvRect,
    lit: bool,
) {
    let registry = world
        .get_resource_mut::<AssetRegistry>()
        .expect("AssetRegistry resource missing");
    registry.register_sprite(
        id.to_string(),
        filter,
        size.0,
        size.1,
        PathBuf::from(format!("test/{id}.png")),
        TextureHandle(page),
        uv,
        lit.then(|| PathBuf::from(format!("test/{id}_n.png"))),
        lit.then(|| PathBuf::from(format!("test/{id}_e.png"))),
        lit.then_some(TextureHandle(page)),
    );
}

const GOLDEN_IDS: [&str; 7] = [
    "page0_a",
    "page0_b",
    "page0_linear",
    "page1",
    "lit",
    "lit_b",
    "unregistered",
];

/// A seeded world for the golden tests: three z layers; unlit sprites on two
/// atlas pages and both filters, lit sprites, materials, three override
/// payloads and parallax layers in every mix; hidden sprites, sprites without
/// `Visibility` and one unregistered ID; and a despawn every fifth spawn, so
/// later entities reuse low IDs and archetype order is not ID order.
fn golden_world(seed: u64, spawns: u32) -> World {
    let mut world = world_with_registry();
    let quarter = |x: f32, y: f32| UvRect {
        min: [x, y],
        max: [x + 0.25, y + 0.5],
    };
    register_on_page(
        &mut world,
        "page0_a",
        FilterMode::Nearest,
        0,
        (16, 16),
        quarter(0.0, 0.0),
        false,
    );
    register_on_page(
        &mut world,
        "page0_b",
        FilterMode::Nearest,
        0,
        (32, 8),
        quarter(0.25, 0.5),
        false,
    );
    register_on_page(
        &mut world,
        "page0_linear",
        FilterMode::Linear,
        0,
        (16, 16),
        quarter(0.5, 0.0),
        false,
    );
    register_on_page(
        &mut world,
        "page1",
        FilterMode::Nearest,
        1,
        (24, 24),
        UvRect::FULL,
        false,
    );
    register_on_page(
        &mut world,
        "lit",
        FilterMode::Nearest,
        7,
        (16, 16),
        quarter(0.0, 0.0),
        true,
    );
    register_on_page(
        &mut world,
        "lit_b",
        FilterMode::Nearest,
        7,
        (16, 32),
        quarter(0.75, 0.5),
        true,
    );
    let mut camera = CameraState::new();
    camera.position = Vec2::new(321.5, -77.25);
    world.insert_resource(camera);

    let golden_ids = GOLDEN_IDS.map(|name| intern(&mut world, name));

    let mut rng = Pcg32::seeded(seed);
    let mut live: Vec<Entity> = Vec::new();
    for index in 0..spawns {
        let e = world.spawn();
        world.insert(
            e,
            Transform {
                position: Vec2::new(rng.next_range(-500.0, 500.0), rng.next_range(-500.0, 500.0)),
                rotation: rng.next_range(-3.0, 3.0),
                scale: Vec2::new(rng.next_range(0.5, 2.5), rng.next_range(0.5, 2.5)),
            },
        );
        let roll = rng.next_u32();
        let id = if roll.is_multiple_of(40) {
            golden_ids[6]
        } else {
            golden_ids[(roll % 6) as usize]
        };
        world.insert(
            e,
            Sprite {
                asset_id: id,
                color: rng.next_u32().to_le_bytes(),
                z_order: [-1, 0, 7][(rng.next_u32() % 3) as usize],
                material_id: match rng.next_u32() % 10 {
                    0 | 1 => Some(MaterialAssetId(0)),
                    2 => Some(MaterialAssetId(1)),
                    _ => None,
                },
            },
        );
        match rng.next_u32() % 30 {
            0 => {}
            1 | 2 => world.insert(e, Visibility { visible: false }),
            _ => world.insert(e, Visibility::default()),
        }
        let payload = rng.next_u32() % 15;
        if payload < 3 {
            let mut block = UniformOverrideBlock::default();
            block.f32s[0] = payload as f32;
            block.vec4[1] = [0.5, payload as f32, 0.0, 1.0];
            world.insert(e, block);
        }
        match rng.next_u32() % 8 {
            0 => world.insert(e, ParallaxLayer::uniform(0.25)),
            1 => world.insert(e, ParallaxLayer::new(Vec2::new(0.5, 1.0))),
            _ => {}
        }
        live.push(e);
        if index % 5 == 4 {
            let victim = live.swap_remove(rng.next_u32() as usize % live.len());
            world.despawn(victim);
        }
    }
    world
}

/// Every field of every batch and instance, in order, as bytes.
fn batch_bytes(batches: &[SpriteBatch]) -> Vec<u8> {
    let mut out = Vec::new();
    for batch in batches {
        out.extend(batch.texture.0.to_le_bytes());
        out.push(match batch.filter {
            FilterMode::Nearest => 0,
            FilterMode::Linear => 1,
        });
        out.extend(batch.material_id.map_or(u32::MAX, |id| id.0).to_le_bytes());
        out.push(u8::from(batch.lit));
        match &batch.uniform_overrides {
            Some(block) => {
                out.push(1);
                out.extend(block.to_bytes());
            }
            None => out.push(0),
        }
        out.extend((batch.instances.len() as u32).to_le_bytes());
        for instance in &batch.instances {
            for value in instance
                .position
                .iter()
                .chain(&instance.size)
                .chain(std::iter::once(&instance.rotation))
                .chain(&instance.uv_min)
                .chain(&instance.uv_size)
                .chain([&instance.z_norm, &instance._pad])
            {
                out.extend(value.to_bits().to_le_bytes());
            }
            out.extend(instance.color);
        }
    }
    out
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

#[track_caller]
fn assert_same_batches(actual: &[SpriteBatch], expected: &[SpriteBatch], context: &str) {
    assert_eq!(actual.len(), expected.len(), "{context}: batch count");
    for (index, (a, e)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(
            a.instances.len(),
            e.instances.len(),
            "{context}: batch {index} instance count"
        );
        assert!(
            batch_bytes(std::slice::from_ref(a)) == batch_bytes(std::slice::from_ref(e)),
            "{context}: batch {index} differs"
        );
    }
}

#[test]
fn golden_world_covers_every_sprite_kind() {
    let world = golden_world(1, 600);
    let batches = reference_extract(&world);
    let instances: usize = batches.iter().map(|b| b.instances.len()).sum();
    assert!(instances > 300, "{instances} instances");
    assert!(batches.iter().any(|b| b.lit));
    assert!(batches.iter().any(|b| b.material_id.is_some()));
    assert!(batches.iter().any(|b| b.uniform_overrides.is_some()));
    assert!(batches.iter().any(|b| b.filter == FilterMode::Linear));
    assert!(batches.iter().any(|b| b.texture == TextureHandle(1)));
    // Three z layers, each opening its own run of batches.
    assert!(batches.len() >= 3 * 5, "{} batches", batches.len());
}

#[test]
fn default_extract_output_is_pinned() {
    // Counts and digest of the pre-rewrite extract on this world.
    let world = golden_world(1, 600);
    let batches = extract_sprites_default(&world);
    let instances: usize = batches.iter().map(|b| b.instances.len()).sum();
    assert_eq!(batches.len(), 69);
    assert_eq!(instances, 420);
    assert_eq!(fnv1a(&batch_bytes(&batches)), 0x7cec_8f23_3d8b_fd57);
    assert_eq!(
        fnv1a(&batch_bytes(&reference_extract(&world))),
        0x7cec_8f23_3d8b_fd57
    );
}

#[test]
fn default_extract_matches_reference_on_seeded_worlds() {
    for seed in 1..=12 {
        let world = golden_world(seed, 150 * seed as u32);
        assert_same_batches(
            &extract_sprites_default(&world),
            &reference_extract(&world),
            &format!("seed {seed}"),
        );
    }
}

#[test]
fn sprites_already_in_painter_order_match_reference() {
    // Spawn order is ID order and z never decreases: the sort is skipped.
    let mut world = world_with_registry();
    register_sprite(&mut world, "quad", FilterMode::Nearest);
    register_lit_sprite(&mut world, "lit_quad");
    for index in 0..300 {
        let id = if index % 7 == 0 { "lit_quad" } else { "quad" };
        spawn_visible(&mut world, id, Vec2::new(index as f32, 2.0), index / 100);
    }
    assert_same_batches(
        &extract_sprites_default(&world),
        &reference_extract(&world),
        "sorted world",
    );
}

#[test]
fn painter_order_sorts_like_z_order_then_entity_id() {
    let mut pairs = Vec::new();
    for z in [i32::MIN, -7, -1, 0, 1, 7, i32::MAX] {
        for id in [0, 1, 99, u32::MAX] {
            pairs.push((z, id));
        }
    }
    for a in &pairs {
        for b in &pairs {
            assert_eq!(
                painter_order(a.0, a.1).cmp(&painter_order(b.0, b.1)),
                a.cmp(b),
                "{a:?} against {b:?}"
            );
        }
    }
}

/// Data pointers of the batches' instance vectors, sorted.
fn instance_storage(batches: &[SpriteBatch]) -> Vec<*const SpriteInstance> {
    let mut pointers: Vec<_> = batches.iter().map(|b| b.instances.as_ptr()).collect();
    pointers.sort();
    pointers
}

#[test]
fn scratch_reuses_storage_and_leaves_output_unchanged() {
    let mut world = golden_world(3, 400);
    let expected = reference_extract(&world);
    world.insert_resource(ExtractScratch::default());

    let first = extract_sprites_default(&world);
    assert_same_batches(&first, &expected, "first frame");
    let storage = instance_storage(&first);
    world
        .get_resource::<ExtractScratch>()
        .unwrap()
        .recycle(first);

    // Steady frames: the same output out of the same allocations.
    for frame in 0..3 {
        let batches = extract_sprites_default(&world);
        assert_same_batches(&batches, &expected, &format!("frame {frame}"));
        assert_eq!(instance_storage(&batches), storage, "frame {frame}");
        world
            .get_resource::<ExtractScratch>()
            .unwrap()
            .recycle(batches);
    }

    // A changed world: nothing of the last frame leaks into the next.
    let victims: Vec<Entity> = world
        .query::<(Entity, &Sprite)>()
        .map(|(e, _)| e)
        .filter(|e| e.id() % 3 == 0)
        .collect();
    for victim in victims {
        world.despawn(victim);
    }
    for index in 0..40 {
        spawn_visible(&mut world, "page1", Vec2::new(index as f32, 0.0), -1);
    }
    let changed = extract_sprites_default(&world);
    assert_same_batches(&changed, &reference_extract(&world), "changed world");
}

#[test]
fn nested_extract_falls_back_to_buffers_of_its_own() {
    let mut world = golden_world(2, 200);
    let expected = reference_extract(&world);
    world.insert_resource(ExtractScratch::default());
    let scratch = world.get_resource::<ExtractScratch>().unwrap();
    let held = scratch.0.borrow_mut();
    assert_same_batches(&extract_sprites_default(&world), &expected, "nested");
    drop(held);
    assert_same_batches(&extract_sprites_default(&world), &expected, "after");
}

#[test]
fn instance_pool_hands_out_fitting_vectors_and_drops_the_rest() {
    let mut pool = InstancePool::default();
    for len in [0, 1, 4, 5, 36, 64, 65, 400_000] {
        let instances = pool.take(len);
        assert!(instances.is_empty());
        assert!(instances.capacity() >= len.max(4), "len {len}");
        assert!(instances.capacity() < 2 * len.max(4), "len {len}");
    }

    let batch = |capacity: usize| {
        let mut batch = SpriteBatch::new(TextureHandle(0), FilterMode::Nearest);
        batch.instances = Vec::with_capacity(capacity);
        batch
            .instances
            .push(SpriteInstance::whole([0.0; 2], [1.0; 2], 0.0, [255; 4]));
        batch
    };
    let mut batches = vec![batch(64), batch(100), batch(4_096), batch(0)];
    let big = batches[2].instances.as_ptr();
    pool.refill(&mut batches);
    assert!(batches.is_empty());

    // 100 slots hold 64 instances, not 100: no pooled vector is ever too small.
    let taken = pool.take(100);
    assert!(taken.is_empty() && taken.capacity() >= 128);
    let mut pooled = [pool.take(40).capacity(), pool.take(40).capacity()];
    pooled.sort_unstable();
    assert_eq!(pooled, [64, 100]);
    // The 4,096-slot vector serves its own class and the one below only.
    assert!(pool.take(8).capacity() < 4_096);
    let reused = pool.take(2_000);
    assert_eq!(reused.as_ptr(), big);

    // Vectors a frame leaves in the pool are freed at the next refill.
    let mut batches = vec![batch(64), batch(64)];
    pool.refill(&mut batches);
    let _ = pool.take(64);
    pool.refill(&mut Vec::new());
    assert!(pool.bins.iter().all(Vec::is_empty));
}

#[test]
fn large_shuffled_worlds_match_reference() {
    // Above `RADIX_SORT_MIN` sprites the keys go through the radix sort.
    for seed in [21, 22] {
        let world = golden_world(seed, 6_000);
        let batches = extract_sprites_default(&world);
        let instances: usize = batches.iter().map(|b| b.instances.len()).sum();
        assert!(instances > 2 * RADIX_SORT_MIN, "{instances} instances");
        assert_same_batches(
            &batches,
            &reference_extract(&world),
            &format!("seed {seed}"),
        );
    }
}

#[test]
fn sort_keys_orders_like_a_comparison_sort() {
    let key = |order: u64, record: u32| SortKey {
        order,
        record,
        class: record % 7,
    };
    let mut rng = Pcg32::seeded(5);
    // Orders that vary in the low bytes only, in the z half only, in bytes
    // that are not adjacent, and in every byte; below and above the radix
    // threshold.
    for mask in [
        0x0000_0000_0000_ffff_u64,
        0x00ff_ffff_0000_0000,
        0x8000_00ff_00ff_0f00,
        u64::MAX,
    ] {
        for len in [0usize, 1, 2, RADIX_SORT_MIN - 1, RADIX_SORT_MIN, 5_000] {
            let mut keys: Vec<SortKey> = (0..len)
                .map(|record| {
                    let random = (u64::from(rng.next_u32()) << 32) | u64::from(rng.next_u32());
                    key(random & mask, record as u32)
                })
                .collect();
            // Distinct orders, as painter orders are.
            keys.sort_unstable_by_key(|key| key.order);
            keys.dedup_by_key(|key| key.order);
            let mut expected = keys.clone();
            // Shuffle.
            for index in (1..keys.len()).rev() {
                keys.swap(index, rng.next_u32() as usize % (index + 1));
            }
            let (set_bits, common_bits) = keys.iter().fold((0, u64::MAX), |(set, common), key| {
                (set | key.order, common & key.order)
            });
            sort_keys(&mut keys, &mut Vec::new(), set_bits ^ common_bits);
            expected.sort_unstable_by_key(|key| key.order);
            let pairs = |keys: &[SortKey]| -> Vec<(u64, u32, u32)> {
                keys.iter().map(|k| (k.order, k.record, k.class)).collect()
            };
            assert_eq!(pairs(&keys), pairs(&expected), "mask {mask:#x} len {len}");
        }
    }
}

/// One texture, one z, spawned in ID order: the whole frame is one batch.
fn single_batch_world(count: u32) -> World {
    let mut world = world_with_registry();
    register_sprite(&mut world, "quad", FilterMode::Nearest);
    for index in 0..count {
        spawn_visible(&mut world, "quad", Vec2::new(index as f32, 1.0), 0);
    }
    world
}

#[test]
fn single_batch_frames_match_reference_as_the_count_changes() {
    let mut world = single_batch_world(500);
    world.insert_resource(ExtractScratch::default());
    let recycle = |world: &World, batches: Vec<SpriteBatch>| {
        world
            .get_resource::<ExtractScratch>()
            .unwrap()
            .recycle(batches);
    };

    // The first frame fills `z_norm` in place; equal counts then reuse what
    // pass 1 wrote, out of the same allocation.
    let first = extract_sprites_default(&world);
    assert_eq!(first.len(), 1);
    assert_same_batches(&first, &reference_extract(&world), "first frame");
    let storage = first[0].instances.as_ptr();
    recycle(&world, first);
    for frame in 0..3 {
        let batches = extract_sprites_default(&world);
        assert_same_batches(
            &batches,
            &reference_extract(&world),
            &format!("frame {frame}"),
        );
        assert_eq!(batches[0].instances.as_ptr(), storage, "frame {frame}");
        recycle(&world, batches);
    }

    // More sprites, fewer sprites, then none: `z_norm` follows the count.
    for index in 0..40 {
        spawn_visible(&mut world, "quad", Vec2::new(index as f32, 9.0), 0);
    }
    let grown = extract_sprites_default(&world);
    assert_same_batches(&grown, &reference_extract(&world), "grown");
    recycle(&world, grown);

    let victims: Vec<Entity> = world
        .query::<(Entity, &Sprite)>()
        .map(|(e, _)| e)
        .filter(|e| e.id() >= 100)
        .collect();
    for victim in victims {
        world.despawn(victim);
    }
    let shrunk = extract_sprites_default(&world);
    assert_same_batches(&shrunk, &reference_extract(&world), "shrunk");
    recycle(&world, shrunk);

    let rest: Vec<Entity> = world.query::<(Entity, &Sprite)>().map(|(e, _)| e).collect();
    for entity in rest {
        world.despawn(entity);
    }
    let empty = extract_sprites_default(&world);
    assert!(empty.is_empty());
    recycle(&world, empty);
    assert!(extract_sprites_default(&world).is_empty());
}

#[test]
fn single_batch_and_split_frames_alternate_cleanly() {
    let mut world = single_batch_world(300);
    register_sprite(&mut world, "other", FilterMode::Linear);
    world.insert_resource(ExtractScratch::default());
    let mut extra = None;
    for frame in 0..6 {
        // Odd frames carry a second class and a second z-run.
        if frame % 2 == 1 {
            extra = Some(spawn_visible(&mut world, "other", Vec2::ZERO, 3));
        } else if let Some(entity) = extra.take() {
            world.despawn(entity);
        }
        let batches = extract_sprites_default(&world);
        assert_eq!(batches.len(), 1 + frame % 2, "frame {frame}");
        assert_same_batches(
            &batches,
            &reference_extract(&world),
            &format!("frame {frame}"),
        );
        world
            .get_resource::<ExtractScratch>()
            .unwrap()
            .recycle(batches);
    }
}

#[test]
fn single_class_out_of_painter_order_still_sorts() {
    // One class and one z, but a despawn and respawn put a low ID last.
    let mut world = single_batch_world(50);
    let first = world
        .query::<(Entity, &Sprite)>()
        .map(|(e, _)| e)
        .next()
        .unwrap();
    world.despawn(first);
    spawn_visible(&mut world, "quad", Vec2::new(-5.0, -5.0), 0);
    let batches = extract_sprites_default(&world);
    assert_eq!(batches.len(), 1);
    assert_same_batches(&batches, &reference_extract(&world), "respawned");
    // One class in order, on two z layers: one batch per z-run.
    let world = {
        let mut world = single_batch_world(20);
        for index in 0..20 {
            spawn_visible(&mut world, "quad", Vec2::new(index as f32, 3.0), 1);
        }
        world
    };
    let batches = extract_sprites_default(&world);
    assert_eq!(batches.len(), 2);
    assert_same_batches(&batches, &reference_extract(&world), "two z-runs");
}

// `D-114`: culling against the view render projects with.

/// A 100 × 100 view at the origin (camera and `WindowSize`) and a 16 × 16
/// quad on page 0.
fn view_world() -> World {
    let mut world = world_with_registry();
    register_sprite(&mut world, "quad", FilterMode::Nearest);
    world.insert_resource(CameraState::new());
    world.insert_resource(WindowSize {
        width: 100,
        height: 100,
    });
    world
}

fn extract_at(world: &World, view: Option<(Vec2, Vec2)>) -> Vec<SpriteBatch> {
    let assets = world.get_resource::<AssetRegistry>().unwrap();
    extract_into(world, assets, view, &mut ExtractBuffers::default())
}

/// The uncull extract's batches less the instances culling drops, and less
/// the batches that leaves empty.
fn uncull_less_culled(world: &World, view: (Vec2, Vec2)) -> Vec<SpriteBatch> {
    let mut batches = extract_at(world, None);
    for batch in &mut batches {
        if batch.material_id.is_none() {
            batch.instances.retain(|i| {
                !outside_view(Vec2::from(i.position), Vec2::from(i.size), i.rotation, view)
            });
        }
    }
    batches.retain(|batch| !batch.instances.is_empty());
    batches
}

fn instance_count(batches: &[SpriteBatch]) -> usize {
    batches.iter().map(|b| b.instances.len()).sum()
}

#[test]
fn culling_keeps_the_uncull_extracts_instances_and_batch_order() {
    // Seeded worlds put culled sprites before, between and after kept ones in
    // every z-run, with lit, material, override and parallax sprites; the
    // larger worlds pass `RADIX_SORT_MIN` keys.
    for seed in 1..=12 {
        let mut world = golden_world(seed, 200 * seed as u32);
        world.insert_resource(WindowSize {
            width: 400,
            height: 300,
        });
        if seed == 12 {
            assert!(world.query::<(Entity, &Sprite)>().count() > RADIX_SORT_MIN);
        }
        let view = view_bounds(&world).unwrap();
        let culled = extract_sprites_default(&world);
        let kept = instance_count(&culled);
        assert!(
            kept > 0 && kept < instance_count(&extract_at(&world, None)),
            "seed {seed}: {kept} kept"
        );
        assert_same_batches(
            &culled,
            &uncull_less_culled(&world, view),
            &format!("seed {seed}"),
        );
    }
}

#[test]
fn culling_with_kept_buffers_matches_as_the_camera_moves() {
    let mut world = golden_world(4, 800);
    world.insert_resource(WindowSize {
        width: 400,
        height: 300,
    });
    world.insert_resource(ExtractScratch::default());
    for frame in 0..4 {
        let view = view_bounds(&world).unwrap();
        let batches = extract_sprites_default(&world);
        assert_same_batches(
            &batches,
            &uncull_less_culled(&world, view),
            &format!("frame {frame}"),
        );
        world
            .get_resource::<ExtractScratch>()
            .unwrap()
            .recycle(batches);
        world.get_resource_mut::<CameraState>().unwrap().position.x -= 150.0;
    }
}

#[test]
fn a_culled_class_opener_keeps_its_batch_ahead() {
    // Same z, in entity order: class A off-screen, class B visible, class A
    // visible. A's batch opened at the culled sprite stays first.
    let mut world = view_world();
    register_on_page(
        &mut world,
        "b",
        FilterMode::Nearest,
        1,
        (16, 16),
        UvRect::FULL,
        false,
    );
    spawn_visible(&mut world, "quad", Vec2::new(-500.0, 10.0), 0);
    spawn_visible(&mut world, "b", Vec2::new(10.0, 10.0), 0);
    spawn_visible(&mut world, "quad", Vec2::new(40.0, 10.0), 0);
    let batches = extract_sprites_default(&world);
    let textures: Vec<u32> = batches.iter().map(|b| b.texture.0).collect();
    assert_eq!(textures, [0, 1], "batch A before batch B");
    assert_eq!(instance_positions(&batches), [[40.0, 10.0], [10.0, 10.0]]);
    let view = view_bounds(&world).unwrap();
    assert_same_batches(&batches, &uncull_less_culled(&world, view), "A, B, A");
}

#[test]
fn a_sprite_wholly_past_an_edge_is_dropped_and_one_reaching_in_is_kept() {
    // The view is (0, 0)–(100, 100); the quad is 16 × 16.
    let cases = [
        (Vec2::new(-16.5, 50.0), false),
        (Vec2::new(-15.0, 50.0), true),
        (Vec2::new(-16.0, 50.0), true),
        (Vec2::new(100.5, 50.0), false),
        (Vec2::new(99.0, 50.0), true),
        (Vec2::new(50.0, -16.5), false),
        (Vec2::new(50.0, -15.0), true),
        (Vec2::new(50.0, 100.5), false),
        (Vec2::new(50.0, 99.0), true),
    ];
    for (position, kept) in cases {
        let mut world = view_world();
        spawn_visible(&mut world, "quad", position, 0);
        assert_eq!(
            instance_count(&extract_sprites_default(&world)),
            usize::from(kept),
            "{position}"
        );
    }
    // A negative scale spans the quad leftward from its position.
    for (x, kept) in [(-1.0, false), (15.0, true)] {
        let mut world = view_world();
        let e = spawn_visible(&mut world, "quad", Vec2::new(x, 50.0), 0);
        world.get_mut::<Transform>(e).unwrap().scale = Vec2::new(-1.0, 1.0);
        assert_eq!(
            instance_count(&extract_sprites_default(&world)),
            usize::from(kept),
            "flipped at {x}"
        );
    }
}

#[test]
fn a_rotated_sprite_whose_corner_reaches_the_view_is_kept() {
    // Turned 45° about its centre (-10, 50), the quad's corner reaches
    // x = 1.3; its unturned box ends at x = -2.
    let mut world = view_world();
    let e = spawn_visible(&mut world, "quad", Vec2::new(-18.0, 42.0), 0);
    world.get_mut::<Transform>(e).unwrap().rotation = std::f32::consts::FRAC_PI_4;
    assert_eq!(instance_count(&extract_sprites_default(&world)), 1);
    // Further out no angle reaches in.
    world.get_mut::<Transform>(e).unwrap().position = Vec2::new(-40.0, 42.0);
    assert_eq!(instance_count(&extract_sprites_default(&world)), 0);
}

#[test]
fn a_parallax_sprite_is_judged_at_its_remapped_position() {
    let mut world = view_world();
    world.get_resource_mut::<CameraState>().unwrap().position = Vec2::new(5_000.0, 0.0);
    let locked = spawn_visible(&mut world, "quad", Vec2::new(10.0, 10.0), 0);
    world.insert(locked, ParallaxLayer::uniform(0.0));
    spawn_visible(&mut world, "quad", Vec2::new(10.0, 10.0), 0);
    let batches = extract_sprites_default(&world);
    assert_eq!(instance_positions(&batches), [[5_010.0, 10.0]]);
}

#[test]
fn camera_zoom_and_rotation_shape_the_culling_view() {
    // Zoom 2 shows (0, 0)–(50, 50).
    let mut world = view_world();
    spawn_visible(&mut world, "quad", Vec2::new(60.0, 10.0), 0);
    assert_eq!(instance_count(&extract_sprites_default(&world)), 1);
    world.get_resource_mut::<CameraState>().unwrap().zoom = 2.0;
    assert_eq!(instance_count(&extract_sprites_default(&world)), 0);

    // A turned camera sees past its unturned box; culling uses the camera's
    // conservative box.
    let mut world = view_world();
    let camera = {
        let camera = world.get_resource_mut::<CameraState>().unwrap();
        camera.rotation = 0.5;
        *camera
    };
    let (min, max) = camera.visible_world_aabb(100.0, 100.0);
    let probe = if min.x < -32.0 {
        Vec2::new(min.x + 1.0, f32::midpoint(min.y, max.y))
    } else {
        Vec2::new(f32::midpoint(min.x, max.x), min.y + 1.0)
    };
    assert!(probe.x + 16.0 < 0.0 || probe.y + 16.0 < 0.0);
    spawn_visible(&mut world, "quad", probe, 0);
    spawn_visible(&mut world, "quad", max + Vec2::ONE, 0);
    assert_eq!(
        instance_positions(&extract_sprites_default(&world)),
        [[probe.x, probe.y]]
    );
}

#[test]
fn an_off_screen_material_sprite_is_kept() {
    let mut world = view_world();
    register_lit_sprite(&mut world, "lit_quad");
    let material = spawn_visible(&mut world, "quad", Vec2::new(-500.0, 10.0), 0);
    world.get_mut::<Sprite>(material).unwrap().material_id = Some(MaterialAssetId(0));
    // Lit wins over a material (`D-061`): this one draws stock and is culled.
    let lit = spawn_visible(&mut world, "lit_quad", Vec2::new(-500.0, 10.0), 0);
    world.get_mut::<Sprite>(lit).unwrap().material_id = Some(MaterialAssetId(0));
    let batches = extract_sprites_default(&world);
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].material_id, Some(MaterialAssetId(0)));
    assert_eq!(instance_positions(&batches), [[-500.0, 10.0]]);
}

#[test]
fn single_batch_frames_cull_and_keep_z_norm_as_the_count_changes() {
    // One class on one z in painter order: the single-batch path.
    let mut world = view_world();
    world.insert_resource(ExtractScratch::default());
    let mut last = None;
    for index in 0..60 {
        last = Some(spawn_visible(
            &mut world,
            "quad",
            Vec2::new(index as f32 * 4.0 - 100.0, 10.0),
            0,
        ));
    }
    let view = view_bounds(&world).unwrap();
    for frame in 0..4 {
        let batches = extract_sprites_default(&world);
        assert_eq!(batches.len(), 1, "frame {frame}");
        assert_same_batches(
            &batches,
            &uncull_less_culled(&world, view),
            &format!("frame {frame}"),
        );
        world
            .get_resource::<ExtractScratch>()
            .unwrap()
            .recycle(batches);
        if frame == 1 {
            world.despawn(last.take().unwrap());
        }
    }
    // Every sprite off-screen: no batch at all.
    world.get_resource_mut::<CameraState>().unwrap().position = Vec2::new(10_000.0, 0.0);
    assert!(extract_sprites_default(&world).is_empty());
}

#[test]
fn the_passed_viewport_wins_over_a_pending_window_size() {
    // A windowed resize writes `WindowSize` at once; the surface, and the
    // projection, follow on `Resized`.
    let mut world = view_world();
    world.insert_resource(WindowSize {
        width: 1_280,
        height: 720,
    });
    spawn_visible(&mut world, "quad", Vec2::new(1_500.0, 10.0), 0);
    assert_eq!(instance_count(&extract_sprites_default(&world)), 0);
    world.insert_resource(ExtractScratch::default());
    world
        .get_resource::<ExtractScratch>()
        .unwrap()
        .set_viewport(1_920, 1_080);
    assert_eq!(instance_count(&extract_sprites_default(&world)), 1);
}

#[test]
fn without_a_viewport_or_a_camera_nothing_is_culled() {
    let mut world = view_world();
    spawn_visible(&mut world, "quad", Vec2::new(-500.0, 10.0), 0);
    world.remove_resource::<WindowSize>();
    assert_eq!(
        instance_count(&extract_sprites_default(&world)),
        1,
        "no viewport"
    );
    world.insert_resource(WindowSize {
        width: 100,
        height: 100,
    });
    world.remove_resource::<CameraState>();
    assert_eq!(
        instance_count(&extract_sprites_default(&world)),
        1,
        "no camera"
    );
}
