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
//! - an `Unlit` one beside the light, drawn grey on the stock pipeline.

use std::path::PathBuf;

use glam::{Vec2, Vec3};
use tungsten::App;
use tungsten::core::components::{SpriteFlip, Unlit};
use tungsten::core::tween::UniformOverrideBlock;
use tungsten::core::{
    AmbientLight, AssetRegistry, Config, Entity, Light, Sprite, Transform, Visibility, World,
};

const ROOT_MANIFEST: &str = "assets/manifest.json";
const LOCAL_MANIFEST: &str = "examples/03_fixtures/assets/manifest.json";

/// The lit sprite every lighting piece draws, from the crate's manifest.
const LIT_SPRITE: &str = "seams_lit";
/// A piece's side on screen: 32 texels at scale 2.
pub const PIECE: f32 = 64.0;
/// The point light at the left of the lit sprite and its twin.
pub const LIGHT_AT: Vec2 = Vec2::new(240.0, 360.0);
/// The pieces' centres.
pub const LIT_AT: Vec2 = Vec2::new(400.0, 240.0);
pub const TWIN_AT: Vec2 = Vec2::new(400.0, 480.0);
pub const FLASH_AT: Vec2 = Vec2::new(560.0, 360.0);
pub const UNLIT_AT: Vec2 = Vec2::new(80.0, 360.0);

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
}
