use super::*;
use crate::testing::Harness;
use crate::{App, DebugHud};
use glam::Vec2;
use std::path::PathBuf;
use tungsten_core::assets::{LayerKind, TilemapRegistry, UvRect};
use tungsten_core::{
    AssetRegistry, Config, FilterMode, Plugin, Schedule, Sprite, SpriteAssetId, TextureHandle,
    TilemapData, TilemapInstance, TilemapLayer, Transform, Visibility,
};
use tungsten_render::SpriteInstance;

fn new_app() -> App {
    let mut app = App::new(Config::default()).expect("App::new failed");
    // The text channel alone in `draw().text`.
    app.world_mut().resource_mut::<DebugHud>().enabled = false;
    app
}

/// Registers a 16 x 16 sprite on atlas page `page`.
fn register(world: &mut World, id: &str, page: u32) -> SpriteAssetId {
    world.resource_mut::<AssetRegistry>().register_sprite(
        id.to_string(),
        FilterMode::Nearest,
        16,
        16,
        PathBuf::from(format!("test/{id}.png")),
        TextureHandle(page),
        UvRect::FULL,
        None,
        None,
        None,
    )
}

fn spawn_sprite(world: &mut World, asset: SpriteAssetId, position: Vec2, z_order: i32) {
    let mut sprite = Sprite::new(asset);
    sprite.z_order = z_order;
    world.spawn_with((
        Transform {
            position,
            rotation: 0.0,
            scale: Vec2::ONE,
        },
        sprite,
        Visibility::default(),
    ));
}

/// A map `width` tiles wide at the origin, one render layer of tileset
/// entry 0, `tile`.
fn add_map(world: &mut World, tile: &str, width: u32, height: u32) {
    world.resource_mut::<TilemapRegistry>().insert(
        "map".to_string(),
        TilemapData {
            tile_width: 16,
            tile_height: 16,
            width,
            height,
            tileset: vec![tile.to_string()],
            layers: vec![TilemapLayer {
                name: "ground".to_string(),
                kind: LayerKind::Render,
                tiles: vec![0; (width * height) as usize],
            }],
        },
    );
    world.spawn_with((TilemapInstance::new("map", Vec2::ZERO),));
}

/// Every field of each batch and of its instances, as text.
fn batches_text(batches: &[SpriteBatch]) -> Vec<String> {
    batches
        .iter()
        .map(|batch| {
            format!(
                "{:?} {:?} {:?} {} {:?} {:?}",
                batch.texture,
                batch.filter,
                batch.material_id,
                batch.lit,
                batch.uniform_overrides,
                batch.instances
            )
        })
        .collect()
}

/// The draw list: one entry per instance with its batch's texture, filter,
/// lighting, material and overrides, so batch boundaries do not count.
fn draw_list(batches: &[SpriteBatch]) -> Vec<String> {
    batches
        .iter()
        .flat_map(|batch| {
            batch.instances.iter().map(move |instance| {
                format!(
                    "{:?} {:?} {} {:?} {:?} {:?}",
                    batch.texture,
                    batch.filter,
                    batch.lit,
                    batch.material_id,
                    batch.uniform_overrides,
                    instance
                )
            })
        })
        .collect()
}

fn debug_text<T: std::fmt::Debug>(items: &[T]) -> Vec<String> {
    items.iter().map(|item| format!("{item:?}")).collect()
}

/// One batch on page `page` with one instance at `z_norm`.
fn batch(page: u32, z_norm: f32) -> SpriteBatch {
    let mut batch = SpriteBatch::new(TextureHandle(page), FilterMode::Linear);
    let mut instance = SpriteInstance::whole([page as f32, 2.5], [3.0, 4.0], 0.25, [1, 2, 3, 4]);
    instance.z_norm = z_norm;
    batch.instances = vec![instance];
    batch
}

fn quad(page: u32) -> QuadInstance {
    QuadInstance {
        position: [page as f32, 1.0],
        size: [2.0, 3.0],
        color: [0.25, 0.5, 0.75, 1.0],
    }
}

fn section(content: &str) -> TextSection {
    TextSection {
        content: content.to_string(),
        font_id: "ui".to_string(),
        font_size: 16.0,
        line_height: 20.0,
        color: [255; 4],
        position: [10.0, 20.0],
        ..TextSection::default()
    }
}

/// Adds one sprite, quad and text contribution, each marked by `page`.
fn contribute(world: &mut World, name: &'static str, page: u32, z_norm: f32) {
    let extracts = world.resource_mut::<Extracts>();
    extracts.add_sprites(move |_| vec![batch(page, z_norm)]);
    extracts.add_quads(move |_| vec![quad(page)]);
    extracts.add_text(move |_| vec![section(name)]);
}

struct PluginA;

impl Plugin for PluginA {
    fn build(&self, _schedule: &mut Schedule, world: &mut World) {
        contribute(world, "a", 10, 0.0);
    }
}

struct PluginB;

impl Plugin for PluginB {
    fn build(&self, _schedule: &mut Schedule, world: &mut World) {
        contribute(world, "b", 11, 0.75);
    }
}

#[test]
fn default_draws_tilemaps_then_sprites() {
    // No map: the frame's batches are the sprite extract's own.
    let mut app = new_app();
    let quad_id = register(app.world_mut(), "quad", 0);
    spawn_sprite(app.world_mut(), quad_id, Vec2::new(40.0, 30.0), 0);
    let mut harness = Harness::new(app);
    harness.step(1);
    let draw = harness.draw().expect("a completed frame");
    assert_eq!(draw.sprites.len(), 1);
    assert_eq!(
        batches_text(&draw.sprites),
        batches_text(&extract_sprites_default(harness.world()))
    );

    // A 2 x 1 map: its tiles at the far plane, then the sprite.
    register(harness.world_mut(), "tile", 1);
    add_map(harness.world_mut(), "tile", 2, 1);
    harness.step(1);
    let draw = harness.draw().expect("a completed frame");
    let mut expected = extract_tilemaps(harness.world());
    let tiles: usize = expected.iter().map(|batch| batch.instances.len()).sum();
    assert_eq!(tiles, 2);
    for tile in expected.iter_mut().flat_map(|batch| &mut batch.instances) {
        tile.z_norm = 1.0;
    }
    expected.extend(extract_sprites_default(harness.world()));
    assert_eq!(draw_list(&draw.sprites), draw_list(&expected));
}

#[test]
fn default_tiles_stay_under_sprites_under_gpu_depth() {
    let mut app = new_app();
    let world = app.world_mut();
    register(world, "tile", 1);
    let quad_id = register(world, "quad", 0);
    add_map(world, "tile", 4, 2);
    spawn_sprite(world, quad_id, Vec2::new(8.0, 8.0), 0);
    spawn_sprite(world, quad_id, Vec2::new(12.0, 12.0), 1);
    let mut harness = Harness::new(app);
    harness.step(1);
    let draw = harness.draw().expect("a completed frame");

    // The extract writes `z_norm` under either sort; `gpu_depth` tests it
    // with `LessEqual` against a 1.0 clear.
    let depths = |page: u32| -> Vec<f32> {
        draw.sprites
            .iter()
            .filter(|batch| batch.texture == TextureHandle(page))
            .flat_map(|batch| batch.instances.iter().map(|instance| instance.z_norm))
            .collect()
    };
    let tiles = depths(1);
    assert_eq!(tiles.len(), 8);
    assert!(tiles.iter().all(|&z| z == 1.0), "{tiles:?}");
    // In draw order: the later-drawn sprite has the lower value.
    let sprites = depths(0);
    assert_eq!(sprites.len(), 2);
    assert!(1.0 > sprites[0] && sprites[0] > sprites[1], "{sprites:?}");
}

#[test]
fn contributions_follow_the_default_in_registration_order() {
    let mut app = new_app();
    app.add_plugin(PluginA);
    app.add_plugin(PluginB);
    contribute(app.world_mut(), "game", 12, 0.0);
    let quad_id = register(app.world_mut(), "quad", 0);
    spawn_sprite(app.world_mut(), quad_id, Vec2::new(40.0, 30.0), 0);
    let mut harness = Harness::new(app);
    harness.step(1);
    let draw = harness.draw().expect("a completed frame");

    // B's batch keeps the nonzero `z_norm` its closure wrote.
    let mut expected = extract_sprites_default(harness.world());
    assert_eq!(expected.len(), 1);
    expected.extend([batch(10, 0.0), batch(11, 0.75), batch(12, 0.0)]);
    assert_eq!(batches_text(&draw.sprites), batches_text(&expected));
    assert_eq!(
        debug_text(&draw.quads),
        debug_text(&[quad(10), quad(11), quad(12)])
    );
    assert_eq!(
        debug_text(&draw.text),
        debug_text(&[section("a"), section("b"), section("game")])
    );
}

#[test]
fn replace_keeps_contributions() {
    // Alone, the replacement is all the channel draws: the sprite in the
    // world went with the default.
    let mut app = new_app();
    let quad_id = register(app.world_mut(), "quad", 0);
    spawn_sprite(app.world_mut(), quad_id, Vec2::new(40.0, 30.0), 0);
    app.set_extract_sprites(|_| vec![batch(20, 0.0)]);
    let mut harness = Harness::new(app);
    harness.step(1);
    let draw = harness.draw().expect("a completed frame");
    assert_eq!(batches_text(&draw.sprites), batches_text(&[batch(20, 0.0)]));

    // With A, added before or after the replace: the replacement's, then A's.
    for plugin_first in [true, false] {
        let mut app = new_app();
        if plugin_first {
            app.add_plugin(PluginA);
        }
        app.set_extract_sprites(|_| vec![batch(20, 0.0)]);
        if !plugin_first {
            app.add_plugin(PluginA);
        }
        let mut harness = Harness::new(app);
        harness.step(1);
        let draw = harness.draw().expect("a completed frame");
        assert_eq!(
            batches_text(&draw.sprites),
            batches_text(&[batch(20, 0.0), batch(10, 0.0)]),
            "plugin first: {plugin_first}"
        );
    }
}

#[test]
fn last_replace_wins() {
    let mut app = new_app();
    app.set_extract_sprites(|_| vec![batch(20, 0.0)]);
    app.set_extract_quads(|_| vec![quad(20)]);
    app.set_extract_text(|_| vec![section("first")]);
    let extracts = app.world_mut().resource_mut::<Extracts>();
    extracts.replace_sprites(|_| vec![batch(21, 0.0)]);
    extracts.replace_quads(|_| vec![quad(21)]);
    extracts.replace_text(|_| vec![section("second")]);
    app.set_extract_text(|_| vec![section("third")]);
    let mut harness = Harness::new(app);
    harness.step(1);
    let draw = harness.draw().expect("a completed frame");
    assert_eq!(batches_text(&draw.sprites), batches_text(&[batch(21, 0.0)]));
    assert_eq!(debug_text(&draw.quads), debug_text(&[quad(21)]));
    assert_eq!(debug_text(&draw.text), debug_text(&[section("third")]));
}
