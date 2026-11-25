//! The `lighting` scene: the M29 smoke row's subject (`D-193`). Five lit
//! sprites, the root manifest's `walk_0`…`walk_4`, whose normal and emissive
//! maps put them on the default extract's lit path, stand in a row through
//! the view's centre beside one unlit quad, so a frame keys a lit and an
//! unlit batch. A dim ambient, a warm and a cool point light at the row's
//! ends and a directional fill light them, with example 01's M29 values. The
//! lights are static: the row checks the lit pipeline, the light upload and
//! the lit/unlit batch keys, which a few frames cover.
//!
//! `TUNGSTEN_LIGHTING_FIXTURE=lamps` draws the subject of the lights' tiles
//! instead (`D-310`): a floor of the crate's `seams_lit` tiles under a dim
//! fill and [`LAMPS`] point lamps, more than the default cap packs, each
//! lighting a disc of [`LAMP_RADIUS`] about a floor tile's centre
//! ([`lamp_at`]). `lamps:turned` also turns and zooms the camera, so a
//! screen tile's box in the world is no square of its own size.
//! `lamps:few` is that view with the fill and the first 15 lamps, the 16
//! lights the block holds, and `lamps:few+1` adds a lamp in view that
//! reaches no floor tile, so the two frames draw the same pixels, the first
//! by the block's loop and the second, under a cap past 16, by the tiles'
//! walk. `tests/integration/seams_regression.rs` counts the lamps lit under
//! a cap, holds the frame with the tiles to the frame without, and the
//! walk to the loop.

use std::f32::consts::FRAC_PI_4;
use std::path::PathBuf;

use glam::{Vec2, Vec3};
use tungsten::App;
use tungsten::core::{
    AmbientLight, AssetRegistry, CameraController, CameraState, Config, Light, Sprite,
    SpriteAssetId, Transform, Visibility, World,
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

/// The lamp grid's floor tile, from the crate's manifest.
const FLOOR_ID: &str = "seams_lit";
const FLOOR_TEXELS: f32 = 32.0;
/// The floor's tiles a row and a column: past the view at 1280 by 720.
const FLOOR: (usize, usize) = (20, 12);
/// The lamp grid's point lamps: more than a cap of 16 packs.
pub const LAMPS: usize = 40;
/// A lamp's radius: short of its neighbours' centres, two floor tiles off.
pub const LAMP_RADIUS: f32 = 60.0;

/// Where lamp `i` of the grid stands: the centre of a floor tile, eight a
/// row on every second tile and every second row, each odd row a tile to
/// the right, so the grid mirrors onto itself in neither axis.
#[must_use]
pub fn lamp_at(i: usize) -> Vec2 {
    let (column, row) = (i % 8, i / 8);
    let tile = Vec2::new((2 + 2 * column + row % 2) as f32, (1 + 2 * row) as f32);
    (tile + Vec2::splat(0.5)) * FLOOR_TEXELS * SCALE
}

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
    match std::env::var("TUNGSTEN_LIGHTING_FIXTURE").as_deref() {
        Err(_) => app.on_startup(|world, _renderer| spawn_scene(world)),
        Ok("lamps") => app.on_startup(|world, _renderer| spawn_lamps(world, LAMPS, false, false)),
        Ok("lamps:turned") => {
            app.on_startup(|world, _renderer| spawn_lamps(world, LAMPS, true, false));
        }
        Ok("lamps:few") => app.on_startup(|world, _renderer| spawn_lamps(world, 15, true, false)),
        Ok("lamps:few+1") => app.on_startup(|world, _renderer| spawn_lamps(world, 15, true, true)),
        Ok(other) => anyhow::bail!(
            "invalid TUNGSTEN_LIGHTING_FIXTURE='{other}': expected lamps, lamps:turned, lamps:few or lamps:few+1"
        ),
    }
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

/// The lamp grid: the floor, a dim fill and the first `lamps` lamps, in
/// five colours, the view showing the floor's 720 rows whatever the window's
/// size. `turned` turns the camera a fifth of a radian about the view's top
/// left and zooms it in by a quarter. `aside` adds a lamp left of the floor,
/// in the turned view and out of reach of every floor tile.
fn spawn_lamps(world: &mut World, lamps: usize, turned: bool, aside: bool) {
    for row in 0..FLOOR.1 {
        for column in 0..FLOOR.0 {
            let centre = (Vec2::new(column as f32, row as f32) + Vec2::splat(0.5)) * FLOOR_TEXELS;
            spawn_sprite(world, FLOOR_ID, FLOOR_TEXELS, centre * SCALE);
        }
    }
    world.insert_resource(AmbientLight(Vec3::splat(0.05)));
    let mut fill = Light::directional(Vec3::splat(0.7), -FRAC_PI_4);
    fill.intensity = 0.05;
    spawn_light(world, fill, Vec2::ZERO);
    const COLOURS: [Vec3; 5] = [
        Vec3::new(1.0, 0.62, 0.35),
        Vec3::new(0.4, 0.65, 1.0),
        Vec3::new(0.55, 1.0, 0.5),
        Vec3::new(1.0, 0.45, 0.8),
        Vec3::new(1.0, 0.95, 0.7),
    ];
    for i in 0..lamps {
        spawn_light(
            world,
            Light::point(COLOURS[i % COLOURS.len()], LAMP_RADIUS),
            lamp_at(i),
        );
    }
    if aside {
        spawn_light(
            world,
            Light::point(Vec3::ONE, LAMP_RADIUS),
            Vec2::new(-100.0, 400.0),
        );
    }
    // The grid fills a window of any size: the view shows the floor's 720
    // rows top to bottom, and the turned view four fifths of them. The zoom
    // is the rig's, since the camera's update writes the state's from it;
    // the turn is the state's, which the update keeps.
    if let Some(controller) = world.get_resource_mut::<CameraController>() {
        controller.rig.view_height = Some(720.0);
        if turned {
            controller.rig.zoom = 1.25;
        }
    }
    if turned && let Some(camera) = world.get_resource_mut::<CameraState>() {
        camera.rotation = 0.2;
    }
}
