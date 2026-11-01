//! The `seams` scene (redo 7.9): one piece for each drawing seam of the
//! boot-and-seams work, which `tests/integration/seams_regression.rs` checks
//! by its pixels; the scene's capture is the drawing sheet. No variables of
//! its own.
//!
//! Every lighting piece draws `seams_lit`, a 32 px sprite at scale 2:
//! `assets/seams/lit.png` is sRGB grey 128, its normal map `lit_n.png` faces
//! left, `(−0.8, 0, 0.6)`, on its left half and right, `(0.8, 0, 0.6)`, on its
//! right half, and its emissive mask `lit_e.png` is black but for a red column
//! down its right edge, texels 28 to 31. A point light stands at the left of
//! the lit sprite and its flipped twin, and a dim ambient lights the rest:
//!
//! - the lit sprite, and its twin under a `SpriteFlip`, whose image and
//!   normals mirror, so its lit side stays toward the light and its red edge
//!   moves to its left;
//! - a lit sprite under a `UniformOverrideBlock` with the flash lanes full
//!   white, which whitens it;
//! - an `Unlit` one beside the light, drawn grey on the stock pipeline;
//! - a strip of `seams_white` tiles, a map layer at `z` 1, between a red
//!   sprite at `z_order` 0, which it covers, and a yellow one at 2, which
//!   covers it;
//! - a layer of `seams_lit` tiles beside the light, unlit and tinted green,
//!   so it draws flat;
//! - a `Backdrop` of `seams_white` tinted dark teal, repeated over the view
//!   behind everything.

use std::path::PathBuf;

use glam::{Vec2, Vec3};
use tungsten::App;
use tungsten::core::assets::tilemap::LayerDraw;
use tungsten::core::assets::{LayerKind, TilemapRegistry};
use tungsten::core::components::{Backdrop, BackdropRepeat, SpriteFlip, Unlit};
use tungsten::core::tween::UniformOverrideBlock;
use tungsten::core::{
    AmbientLight, AssetRegistry, Config, Entity, Light, Sprite, TilemapData, TilemapInstance,
    Transform, Visibility, World,
};

const ROOT_MANIFEST: &str = "assets/manifest.json";
const LOCAL_MANIFEST: &str = "examples/03_fixtures/assets/manifest.json";

/// The lit sprite every lighting piece draws, from the crate's manifest.
const LIT_SPRITE: &str = "seams_lit";
/// A plain white 32 px sprite, tinted by what draws it.
const WHITE_SPRITE: &str = "seams_white";
/// A piece's side on screen: 32 texels at scale 2.
pub const PIECE: f32 = 64.0;
/// The point light at the left of the lit sprite and its twin.
pub const LIGHT_AT: Vec2 = Vec2::new(240.0, 360.0);
/// The pieces' centres.
pub const LIT_AT: Vec2 = Vec2::new(400.0, 240.0);
pub const TWIN_AT: Vec2 = Vec2::new(400.0, 480.0);
pub const FLASH_AT: Vec2 = Vec2::new(560.0, 360.0);
pub const UNLIT_AT: Vec2 = Vec2::new(80.0, 360.0);
/// The strip's top-left corner: three 32 px tiles across.
pub const STRIP_AT: Vec2 = Vec2::new(736.0, 112.0);
/// The sprites the strip lies between, 32 px squares: the red one under it,
/// overlapping its left tile's top half, and the yellow one over it,
/// overlapping its right tile's bottom half.
pub const UNDER_STRIP_AT: Vec2 = Vec2::new(752.0, 96.0);
pub const OVER_STRIP_AT: Vec2 = Vec2::new(800.0, 128.0);
pub const RED: [u8; 4] = [220, 40, 40, 255];
pub const YELLOW: [u8; 4] = [230, 210, 40, 255];
/// The tinted unlit layer's top-left corner: two `seams_lit` tiles across,
/// within the light's reach.
pub const GLASS_AT: Vec2 = Vec2::new(160.0, 560.0);
pub const GLASS_TINT: [u8; 4] = [128, 255, 128, 255];
/// The backdrop's tint.
pub const BACKDROP_TINT: [u8; 4] = [8, 40, 48, 255];

/// Runs the `seams` scene: the root config and both manifests, no systems of
/// its own.
pub fn run() -> anyhow::Result<()> {
    let mut config = Config::load("tungsten.json")?;
    config.window.title = "Fixtures — seams (redo 7.9)".to_string();

    let mut app = App::new(config)?;
    app.set_manifest_roots(vec![
        PathBuf::from(ROOT_MANIFEST),
        PathBuf::from(LOCAL_MANIFEST),
    ]);
    app.on_startup(|world, _renderer| spawn_scene(world));
    app.run()
}

/// Spawns `seams_lit` centred on `centre`.
fn spawn_piece(world: &mut World, centre: Vec2) -> Entity {
    let asset = world
        .resource_mut::<AssetRegistry>()
        .intern_sprite(LIT_SPRITE);
    world.spawn_with((
        Transform {
            position: centre - Vec2::splat(PIECE * 0.5),
            rotation: 0.0,
            scale: Vec2::splat(PIECE / 32.0),
        },
        Sprite::new(asset),
        Visibility::default(),
    ))
}

fn spawn_scene(world: &mut World) {
    spawn_piece(world, LIT_AT);
    let twin = spawn_piece(world, TWIN_AT);
    world.insert(twin, SpriteFlip(true));
    let flash = spawn_piece(world, FLASH_AT);
    world.insert(
        flash,
        UniformOverrideBlock::from_slots(
            [[1.0; 4], [0.0; 4], [0.0; 4], [0.0; 4]],
            [1.0, 0.0, 0.0, 0.0],
            [0; 4],
        ),
    );
    let unlit = spawn_piece(world, UNLIT_AT);
    world.insert(unlit, Unlit);

    world.insert_resource(AmbientLight(Vec3::splat(0.1)));
    let mut light = Light::point(Vec3::ONE, 480.0);
    light.intensity = 2.0;
    world.spawn_with((Transform::from_position(LIGHT_AT), light));

    spawn_map(
        world,
        "seams_strip",
        WHITE_SPRITE,
        3,
        STRIP_AT,
        LayerDraw {
            z: Some(1),
            ..LayerDraw::DEFAULT
        },
    );
    spawn_tinted(world, UNDER_STRIP_AT, RED, 0);
    spawn_tinted(world, OVER_STRIP_AT, YELLOW, 2);
    spawn_map(
        world,
        "seams_glass",
        LIT_SPRITE,
        2,
        GLASS_AT,
        LayerDraw {
            unlit: true,
            tint: GLASS_TINT,
            ..LayerDraw::DEFAULT
        },
    );

    let white = world
        .resource_mut::<AssetRegistry>()
        .intern_sprite(WHITE_SPRITE);
    world.spawn_with((
        Transform::default(),
        Backdrop {
            tint: BACKDROP_TINT,
            repeat: BackdropRepeat::Xy,
            ..Backdrop::new(white)
        },
    ));
}

/// Spawns `seams_white` at 32 px, its top-left at `at`, tinted `color`, at
/// `z_order`.
fn spawn_tinted(world: &mut World, at: Vec2, color: [u8; 4], z_order: i32) {
    let asset = world
        .resource_mut::<AssetRegistry>()
        .intern_sprite(WHITE_SPRITE);
    let mut sprite = Sprite::new(asset);
    sprite.color = color;
    sprite.z_order = z_order;
    world.spawn_with((Transform::from_position(at), sprite, Visibility::default()));
}

/// Registers map `id`, one row of `tiles` 32 px tiles of `sprite` in one
/// render layer drawn as `draw`, and spawns it at `at`.
fn spawn_map(world: &mut World, id: &str, sprite: &str, tiles: u32, at: Vec2, draw: LayerDraw) {
    let mut map = TilemapData::new(32, 32, tiles, 1);
    let tile = map.add_tile(sprite);
    let layer = map.add_layer("tiles", LayerKind::Render);
    map.fill(layer, tile);
    map.set_layer_draw(layer, draw);
    if !world.has_resource::<TilemapRegistry>() {
        world.insert_resource(TilemapRegistry::new());
    }
    world
        .resource_mut::<TilemapRegistry>()
        .insert(id.to_string(), map);
    world.spawn_with((TilemapInstance::new(id, at),));
}
