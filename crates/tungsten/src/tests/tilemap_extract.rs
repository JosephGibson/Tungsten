use super::*;
use glam::Vec2;
use std::collections::HashMap;
use std::path::PathBuf;
use tungsten_core::assets::{TilemapData, TilemapLayer, UvRect};

fn register(world: &mut World, id: &str, page: u32, filter: FilterMode, uv: UvRect) {
    register_as(world, id, page, filter, uv, false);
}

/// As [`register`], with a normal map when `lit`: the loader gives such a
/// sprite a lit atlas, under its albedo page's handle.
fn register_as(world: &mut World, id: &str, page: u32, filter: FilterMode, uv: UvRect, lit: bool) {
    world
        .get_resource_mut::<AssetRegistry>()
        .expect("AssetRegistry resource missing")
        .register_sprite(
            id.to_string(),
            filter,
            16,
            16,
            PathBuf::from(format!("test/{id}.png")),
            TextureHandle(page),
            uv,
            lit.then(|| PathBuf::from(format!("test/{id}_n.png"))),
            None,
            lit.then_some(TextureHandle(page)),
        );
}

fn layer(name: &str, kind: LayerKind, tiles: Vec<i32>) -> TilemapLayer {
    TilemapLayer {
        name: name.to_string(),
        kind,
        tiles,
    }
}

/// A world with a 1280 x 720 window, a camera at `camera` and one 16 px
/// tilemap, `width` tiles wide, at the origin.
fn world_with_map(camera: Vec2, tileset: &[&str], width: u32, layers: Vec<TilemapLayer>) -> World {
    let mut world = World::new();
    world.insert_resource(AssetRegistry::new());
    world.insert_resource(WindowSize {
        width: 1280,
        height: 720,
    });
    let mut state = CameraState::new();
    state.position = camera;
    world.insert_resource(state);

    let height = layers[0].tiles.len() as u32 / width;
    let mut tilemaps = TilemapRegistry::new();
    tilemaps.insert(
        "map".to_string(),
        TilemapData {
            tile_width: 16,
            tile_height: 16,
            width,
            height,
            tileset: tileset.iter().map(ToString::to_string).collect(),
            layers,
        },
    );
    world.insert_resource(tilemaps);
    let e = world.spawn();
    world.insert(e, TilemapInstance::new("map", Vec2::ZERO));
    world
}

/// Every field of a batch and of its instances, as text.
fn batch_text(batch: &SpriteBatch) -> String {
    format!(
        "{:?} {:?} {:?} {} {:?} {:?}",
        batch.texture,
        batch.filter,
        batch.material_id,
        batch.lit,
        batch.uniform_overrides,
        batch.instances
    )
}

fn batches_text(batches: &[SpriteBatch]) -> Vec<String> {
    batches.iter().map(batch_text).collect()
}

/// The tilemap extract as it stood before tileset entries were cached and
/// the per-layer map became a first-seen list. A layer's batches come out of
/// a `HashMap`, so their order within the layer is arbitrary.
fn reference_extract(world: &World) -> Vec<Vec<SpriteBatch>> {
    let tilemaps = world.get_resource::<TilemapRegistry>().unwrap();
    let assets = world.get_resource::<AssetRegistry>().unwrap();
    let camera = world
        .get_resource::<CameraState>()
        .copied()
        .unwrap_or_default();
    let window = world.get_resource::<WindowSize>().copied().unwrap();
    let (view_min, view_max) = camera.visible_world_aabb(window.width as f32, window.height as f32);

    let mut layers_out = Vec::new();
    for (_entity, instance) in world.query::<(Entity, &TilemapInstance)>() {
        let data = tilemaps.get(&instance.id).unwrap();
        let tw = data.tile_width as f32;
        let th = data.tile_height as f32;
        let col_start = ((view_min.x - instance.origin.x) / tw).floor().max(0.0) as u32;
        let row_start = ((view_min.y - instance.origin.y) / th).floor().max(0.0) as u32;
        let col_end =
            (((view_max.x - instance.origin.x) / tw).ceil().max(0.0) as u32).min(data.width);
        let row_end =
            (((view_max.y - instance.origin.y) / th).ceil().max(0.0) as u32).min(data.height);
        if col_start >= col_end || row_start >= row_end {
            continue;
        }
        for layer in &data.layers {
            if layer.kind != LayerKind::Render {
                continue;
            }
            let mut per_texture: HashMap<u32, SpriteBatch> = HashMap::new();
            for row in row_start..row_end {
                for col in col_start..col_end {
                    let tile = layer.tiles[(row as usize) * (data.width as usize) + (col as usize)];
                    if tile < 0 {
                        continue;
                    }
                    let Some(sprite_id) = data.tileset.get(tile as usize) else {
                        continue;
                    };
                    let Some(asset) = assets.get_sprite(sprite_id) else {
                        continue;
                    };
                    per_texture
                        .entry(asset.atlas.0)
                        .or_insert_with(|| SpriteBatch::new(asset.atlas, asset.filter))
                        .instances
                        .push(SpriteInstance {
                            position: [
                                instance.origin.x + (col as f32) * tw,
                                instance.origin.y + (row as f32) * th,
                            ],
                            size: [tw, th],
                            rotation: 0.0,
                            color: [255; 4],
                            uv_min: asset.uv.min,
                            uv_size: [
                                asset.uv.max[0] - asset.uv.min[0],
                                asset.uv.max[1] - asset.uv.min[1],
                            ],
                            z_norm: 0.0,
                            _pad: 0.0,
                        });
                }
            }
            if !per_texture.is_empty() {
                layers_out.push(per_texture.into_values().collect());
            }
        }
    }
    layers_out
}

const HALF: UvRect = UvRect {
    min: [0.5, 0.0],
    max: [1.0, 0.5],
};

/// 96 x 60 tiles, larger than the view on both axes: three render layers on
/// one atlas page (one sparse, one empty), a collision layer, an unregistered
/// tileset entry and a tile index past the tileset.
fn single_page_world(camera: Vec2) -> World {
    let (width, height) = (96u32, 60u32);
    let count = (width * height) as usize;
    let ground: Vec<i32> = (0..count).map(|i| (i % 3) as i32).collect();
    let detail: Vec<i32> = (0..count)
        .map(|i| match i % 7 {
            0 => 1,
            3 => 3,
            5 => 9,
            _ => -1,
        })
        .collect();
    let mut world = world_with_map(
        camera,
        &["grass", "stone", "water", "unregistered"],
        width,
        vec![
            layer("ground", LayerKind::Render, ground),
            layer("solid", LayerKind::Collision, vec![0; count]),
            layer("detail", LayerKind::Render, detail),
            layer("empty", LayerKind::Render, vec![-1; count]),
        ],
    );
    register(&mut world, "grass", 0, FilterMode::Nearest, UvRect::FULL);
    register(&mut world, "stone", 0, FilterMode::Nearest, HALF);
    register(&mut world, "water", 0, FilterMode::Linear, HALF);
    world
}

#[test]
fn single_page_layers_match_reference() {
    for camera in [
        Vec2::new(640.0, 360.0),
        Vec2::new(700.5, 512.25),
        Vec2::new(-200.0, 100.0),
        Vec2::new(5000.0, 5000.0),
    ] {
        let world = single_page_world(camera);
        let batches = extract_tilemaps(&world);
        let reference: Vec<SpriteBatch> = reference_extract(&world).into_iter().flatten().collect();
        assert_eq!(
            batches_text(&batches),
            batches_text(&reference),
            "camera {camera}"
        );
    }
    // In view: one batch for each of the two layers that draw tiles.
    let world = single_page_world(Vec2::new(640.0, 360.0));
    assert_eq!(extract_tilemaps(&world).len(), 2);
}

/// Two layers that alternate between two atlas pages, all in view: the first
/// tile of layer "a" is on page 0, that of layer "b" on page 1.
fn two_page_world() -> World {
    let (width, height) = (40u32, 20u32);
    let tiles: Vec<i32> = (0..(width * height) as i32)
        .map(|i| i32::from(i % 5 != 0))
        .collect();
    let mut flipped = tiles.clone();
    flipped[0] = 1;
    let mut world = world_with_map(
        Vec2::ZERO,
        &["page0_tile", "page1_tile"],
        width,
        vec![
            layer("a", LayerKind::Render, tiles),
            layer("b", LayerKind::Render, flipped),
        ],
    );
    register(
        &mut world,
        "page0_tile",
        0,
        FilterMode::Nearest,
        UvRect::FULL,
    );
    register(&mut world, "page1_tile", 1, FilterMode::Linear, HALF);
    world
}

#[test]
fn layer_on_two_pages_opens_batches_in_first_seen_order() {
    let world = two_page_world();
    let batches = extract_tilemaps(&world);
    let pages: Vec<u32> = batches.iter().map(|batch| batch.texture.0).collect();
    assert_eq!(pages, vec![0, 1, 1, 0]);

    // Same batches as the reference, whose order within a layer is arbitrary.
    let reference = reference_extract(&world);
    assert_eq!(reference.len(), 2);
    for (index, expected) in reference.iter().enumerate() {
        let mut actual = batches_text(&batches[2 * index..2 * index + 2]);
        let mut expected = batches_text(expected);
        actual.sort();
        expected.sort();
        assert_eq!(actual, expected, "layer {index}");
    }
}

#[test]
fn tileset_entry_resolves_once_per_map() {
    let mut world = World::new();
    world.insert_resource(AssetRegistry::new());
    register(&mut world, "grass", 3, FilterMode::Linear, HALF);
    let assets = world.get_resource::<AssetRegistry>().unwrap();
    let ids = ["grass", "unregistered"];

    let mut cache = TilesetCache::default();
    cache.begin(ids.len());
    let mut lookups = 0;
    for tile in [0usize, 1, 0, 0, 1, 2, 2, 0] {
        let sprite = cache.get(tile, || {
            lookups += 1;
            assets.get_sprite(ids[tile])
        });
        match tile {
            0 => {
                let sprite = sprite.expect("grass resolves");
                assert_eq!(sprite.atlas, TextureHandle(3));
                assert_eq!(sprite.filter, FilterMode::Linear);
                assert_eq!(sprite.uv_min, [0.5, 0.0]);
                assert_eq!(sprite.uv_size, [0.5, 0.5]);
            }
            // An unregistered ID and an index past the tileset draw nothing.
            _ => assert!(sprite.is_none()),
        }
    }
    // One lookup each for entries 0 and 1; index 2 is outside the tileset.
    assert_eq!(lookups, 2);

    // The next map starts unresolved.
    cache.begin(ids.len());
    assert!(
        cache
            .get(0, || {
                lookups += 1;
                assets.get_sprite(ids[0])
            })
            .is_some()
    );
    assert_eq!(lookups, 3);
}

#[test]
fn scratch_reuse_leaves_output_unchanged() {
    let mut world = single_page_world(Vec2::new(640.0, 360.0));
    let expected = batches_text(&extract_tilemaps(&world));
    world.insert_resource(ExtractScratch::default());

    for frame in 0..3 {
        let batches = extract_tilemaps(&world);
        assert_eq!(batches_text(&batches), expected, "frame {frame}");
        let scratch = world.get_resource::<ExtractScratch>().unwrap();
        scratch.recycle(batches);
    }

    // A view of other tiles: batch sizes change, nothing of the last frame leaks.
    world.get_resource_mut::<CameraState>().unwrap().position = Vec2::new(900.0, 700.0);
    let moved = batches_text(&extract_tilemaps(&world));
    world.remove_resource::<ExtractScratch>();
    assert_eq!(moved, batches_text(&extract_tilemaps(&world)));
    assert_ne!(moved, expected);
}

/// Two instances of one 4 x 2 map, the second 160 px below the first. Its
/// render layers `back`, `mid` and `front`, in that file order around a
/// collision layer, each draw one sprite on a page of their own: 10, 11, 12.
fn two_map_world() -> World {
    let mut world = world_with_map(
        Vec2::ZERO,
        &["back_tile", "mid_tile", "front_tile"],
        4,
        vec![
            layer("back", LayerKind::Render, vec![0; 8]),
            layer("solid", LayerKind::Collision, vec![0; 8]),
            layer("mid", LayerKind::Render, vec![1; 8]),
            layer("front", LayerKind::Render, vec![2; 8]),
        ],
    );
    let second = world.spawn();
    world.insert(second, TilemapInstance::new("map", Vec2::new(0.0, 160.0)));
    register(
        &mut world,
        "back_tile",
        10,
        FilterMode::Nearest,
        UvRect::FULL,
    );
    register(
        &mut world,
        "mid_tile",
        11,
        FilterMode::Nearest,
        UvRect::FULL,
    );
    register(&mut world, "front_tile", 12, FilterMode::Nearest, HALF);
    world
}

#[test]
fn layer_form_draws_named_render_layers_in_map_order() {
    let world = two_map_world();
    let all = extract_tilemaps(&world);
    let pages: Vec<u32> = all.iter().map(|batch| batch.texture.0).collect();
    assert_eq!(pages, [10, 11, 12, 10, 11, 12]);
    assert_eq!(all[0].instances[0].position, [0.0, 0.0]);
    assert_eq!(all[3].instances[0].position, [0.0, 160.0]);

    // Map by map, each map's layers in file order whatever the names' order.
    // An unknown name draws nothing, and a collision layer never draws.
    let named = extract_tilemap_layers(&world, &["front", "missing", "solid", "back"]);
    let expected: Vec<String> = all
        .iter()
        .filter(|batch| batch.texture.0 != 11)
        .map(batch_text)
        .collect();
    assert_eq!(batches_text(&named), expected);

    assert!(extract_tilemap_layers(&world, &["missing", "solid"]).is_empty());
    assert!(extract_tilemap_layers(&world, &[]).is_empty());
}

/// One 8 x 2 map on one atlas page. Layer `mixed` alternates a lit sprite
/// (even columns) with an unlit one (odd columns); layer `plain` is all unlit.
fn lit_world() -> World {
    let mixed: Vec<i32> = (0..16).map(|i| i % 2).collect();
    let mut world = world_with_map(
        Vec2::ZERO,
        &["lit_tile", "plain_tile"],
        8,
        vec![
            layer("mixed", LayerKind::Render, mixed),
            layer("plain", LayerKind::Render, vec![1; 16]),
        ],
    );
    register_as(
        &mut world,
        "lit_tile",
        0,
        FilterMode::Nearest,
        UvRect::FULL,
        true,
    );
    register(&mut world, "plain_tile", 0, FilterMode::Nearest, HALF);
    world
}

#[test]
fn lit_tiles_batch_apart_from_unlit_on_one_page() {
    let world = lit_world();
    let batches = extract_tilemaps(&world);
    let shape: Vec<(u32, bool, usize)> = batches
        .iter()
        .map(|batch| (batch.texture.0, batch.lit, batch.instances.len()))
        .collect();
    assert_eq!(shape, [(0, true, 8), (0, false, 8), (0, false, 16)]);

    // Row by row: the lit batch holds the even columns, the unlit the odd.
    let columns = |batch: &SpriteBatch| -> Vec<[f32; 2]> {
        batch.instances.iter().map(|tile| tile.position).collect()
    };
    let cells = |first: u32| -> Vec<[f32; 2]> {
        (0..2u32)
            .flat_map(|row| {
                (first..8)
                    .step_by(2)
                    .map(move |col| [col as f32 * 16.0, row as f32 * 16.0])
            })
            .collect()
    };
    assert_eq!(columns(&batches[0]), cells(0));
    assert_eq!(columns(&batches[1]), cells(1));
    assert!(
        batches[0]
            .instances
            .iter()
            .all(|tile| tile.uv_min == [0.0, 0.0])
    );
    assert!(
        batches[1]
            .instances
            .iter()
            .all(|tile| tile.uv_min == [0.5, 0.0])
    );

    // An unlit layer keeps the bytes it had before tiles could be lit.
    let reference = reference_extract(&world);
    assert_eq!(batches_text(&batches[2..]), batches_text(&reference[1]));
}

#[test]
fn every_render_layer_name_matches_extract_tilemaps() {
    for camera in [
        Vec2::new(640.0, 360.0),
        Vec2::new(700.5, 512.25),
        Vec2::new(-200.0, 100.0),
    ] {
        let world = single_page_world(camera);
        assert_eq!(
            batches_text(&extract_tilemap_layers(
                &world,
                &["ground", "solid", "detail", "empty"]
            )),
            batches_text(&extract_tilemaps(&world)),
            "camera {camera}"
        );
    }
    for (world, names) in [
        (two_page_world(), &["b", "a"][..]),
        (two_map_world(), &["back", "mid", "front"][..]),
        (lit_world(), &["mixed", "plain"][..]),
    ] {
        assert_eq!(
            batches_text(&extract_tilemap_layers(&world, names)),
            batches_text(&extract_tilemaps(&world)),
            "{names:?}"
        );
    }
}
