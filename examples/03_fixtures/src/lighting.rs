//! The `lighting` scene: the M29 smoke row's subject (`D-193`). Five lit
//! sprites, the root manifest's `walk_0`…`walk_4`, whose normal and emissive
//! maps put them on the default extract's lit path, stand in a row through
//! the view's centre beside one unlit quad, so a frame keys a lit and an
//! unlit batch. A dim ambient, a warm and a cool point light at the row's
//! ends and a directional fill light them, with example 01's M29 values. The
//! lights are static: the row checks the lit pipeline, the light upload and
//! the lit/unlit batch keys, which a few frames cover.

use std::f32::consts::FRAC_PI_4;
use std::path::PathBuf;

use glam::{Vec2, Vec3};
use tungsten::App;
use tungsten::core::{
    AmbientLight, AssetRegistry, Config, Light, Sprite, SpriteAssetId, Transform, Visibility, World,
};

const ROOT_MANIFEST: &str = "assets/manifest.json";
const LOCAL_MANIFEST: &str = "examples/03_fixtures/assets/manifest.json";

/// The lit sprites, from the root manifest.
const LIT_IDS: [&str; 5] = ["walk_0", "walk_1", "walk_2", "walk_3", "walk_4"];
/// The unlit quad, from the crate's manifest.
const UNLIT_ID: &str = "ex03_quad";
const LIT_TEXELS: f32 = 64.0;
const UNLIT_TEXELS: f32 = 16.0;
const SCALE: f32 = 2.0;
const VIEW_CENTER: Vec2 = Vec2::new(640.0, 360.0);
/// Distance between the centres of neighbouring sprites in the row.
const SPACING: f32 = 150.0;

/// Runs the `lighting` scene: the root config and both manifests, no systems
/// of its own.
pub fn run() -> anyhow::Result<()> {
    let mut config = Config::load("tungsten.json")?;
    config.window.title = "Fixtures — lighting (M29)".to_string();

    let mut app = App::new(config)?;
    app.set_manifest_roots(vec![
        PathBuf::from(ROOT_MANIFEST),
        PathBuf::from(LOCAL_MANIFEST),
    ]);
    app.on_startup(|world, _renderer| spawn_scene(world));
    app.run()
}

/// The centre of slot `i` of the row's six: five lit sprites, then the quad.
fn slot_center(i: usize) -> Vec2 {
    VIEW_CENTER + Vec2::new((i as f32 - 2.5) * SPACING, 0.0)
}

fn sprite_id(world: &mut World, name: &str) -> SpriteAssetId {
    world
        .get_resource_mut::<AssetRegistry>()
        .expect("AssetRegistry resource missing")
        .intern_sprite(name)
}

/// Spawns a sprite `texels` wide at `SCALE`, centred on `center`.
fn spawn_sprite(world: &mut World, name: &str, texels: f32, center: Vec2) {
    let asset = sprite_id(world, name);
    let entity = world.spawn();
    world.insert(
        entity,
        Transform {
            position: center - Vec2::splat(texels * SCALE * 0.5),
            rotation: 0.0,
            scale: Vec2::splat(SCALE),
        },
    );
    world.insert(entity, Sprite::new(asset));
    world.insert(entity, Visibility::default());
}

fn spawn_light(world: &mut World, light: Light, position: Vec2) {
    let entity = world.spawn();
    world.insert(entity, Transform::from_position(position));
    world.insert(entity, light);
}

fn spawn_scene(world: &mut World) {
    for (i, name) in LIT_IDS.iter().enumerate() {
        spawn_sprite(world, name, LIT_TEXELS, slot_center(i));
    }
    spawn_sprite(world, UNLIT_ID, UNLIT_TEXELS, slot_center(LIT_IDS.len()));

    world.insert_resource(AmbientLight(Vec3::splat(0.15)));
    let mut warm = Light::point(Vec3::new(1.0, 0.62, 0.35), 352.0);
    warm.intensity = 1.0;
    spawn_light(world, warm, slot_center(0));
    let mut cool = Light::point(Vec3::new(0.4, 0.65, 1.0), 352.0);
    cool.intensity = 0.9;
    spawn_light(world, cool, slot_center(LIT_IDS.len()));
    let mut fill = Light::directional(Vec3::splat(0.7), -FRAC_PI_4);
    fill.intensity = 0.3;
    spawn_light(world, fill, Vec2::ZERO);
}
