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
//!   behind everything;
//! - an orange glow, the reserved `engine_white` under the stock
//!   fragment-only material `engine_glow`;
//! - a magenta flat quad, `engine_white` scaled by its `Transform`, and a
//!   cyan segment, `SpriteInstance::segment` from an extract of the scene's
//!   own;
//! - a beam of `seams_ray`, a 16 × 8 strip yellow then blue, laid by
//!   `beam` along a segment from the same extract;
//! - two white UI labels on black panels, `Outline` at the top left in a
//!   named style with a green outline and `Shadow` at the top right in one
//!   with a blue shadow down and right.

use std::path::PathBuf;

use glam::{Vec2, Vec3};
use tungsten::core::assets::tilemap::LayerDraw;
use tungsten::core::assets::{LayerKind, MaterialRegistry, TilemapRegistry};
use tungsten::core::components::{Backdrop, BackdropRepeat, SpriteFlip, Unlit};
use tungsten::core::text::{TextOutline, TextShadow};
use tungsten::core::tween::UniformOverrideBlock;
use tungsten::core::ui::{
    Anchor, LayoutStyle, NamedStyle, StyleOverrides, TextOverrides, Theme, UiLayer, UiTree,
};
use tungsten::core::{
    AmbientLight, AssetRegistry, Config, Entity, Light, Sprite, TilemapData, TilemapInstance,
    Transform, Visibility, World,
};
use tungsten::render::SpriteInstance;
use tungsten::{App, Batches, ENGINE_GLOW, ENGINE_WHITE, Extracts, beam};

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
/// The glow's centre and side, and its colour, linear.
pub const GLOW_AT: Vec2 = Vec2::new(1000.0, 220.0);
pub const GLOW_SIDE: f32 = 96.0;
pub const GLOW_COLOR: [f32; 4] = [1.0, 0.6, 0.2, 1.0];
/// The flat quad's top-left corner and size, magenta.
pub const FLAT_AT: Vec2 = Vec2::new(720.0, 300.0);
pub const FLAT_SIZE: Vec2 = Vec2::new(48.0, 24.0);
/// The segment's ends and width, cyan.
pub const SEGMENT_FROM: Vec2 = Vec2::new(800.0, 300.0);
pub const SEGMENT_TO: Vec2 = Vec2::new(880.0, 360.0);
pub const SEGMENT_WIDTH: f32 = 6.0;
/// The labels' text, outline and shadow colours, the outline's width and
/// the shadow's offset in UI units, and the labels' size.
pub const LABEL_TEXT: [u8; 4] = [255, 255, 255, 255];
pub const OUTLINE: [u8; 4] = [0, 255, 0, 255];
pub const OUTLINE_WIDTH: f32 = 2.0;
pub const SHADOW: [u8; 4] = [0, 0, 255, 255];
pub const SHADOW_OFFSET: Vec2 = Vec2::new(3.0, 3.0);
pub const LABEL_SIZE: [f32; 2] = [32.0, 40.0];
/// The beam's ends and width: its strip's tiles 16 px long.
pub const BEAM_FROM: Vec2 = Vec2::new(720.0, 420.0);
pub const BEAM_TO: Vec2 = Vec2::new(1000.0, 420.0);
pub const BEAM_WIDTH: f32 = 8.0;

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

    spawn_glow(world);
    let texel = world
        .resource_mut::<AssetRegistry>()
        .intern_sprite(ENGINE_WHITE);
    let mut flat = Sprite::new(texel);
    flat.color = [255, 0, 255, 255];
    world.spawn_with((
        Transform {
            position: FLAT_AT,
            rotation: 0.0,
            scale: FLAT_SIZE,
        },
        flat,
        Visibility::default(),
    ));
    world.resource_mut::<Extracts>().add_sprites(lines);
    spawn_labels(world.resource_mut::<UiTree>());
}

/// The outlined and the shadowed label, each on a black panel in a corner:
/// the theme names their styles.
fn spawn_labels(tree: &mut UiTree) {
    let mut theme = Theme::default();
    let text = TextOverrides {
        size: Some(LABEL_SIZE[0]),
        line_height: Some(LABEL_SIZE[1]),
        color: Some(LABEL_TEXT),
        ..TextOverrides::default()
    };
    let styles = [
        (
            "seams_outlined",
            NamedStyle {
                text: text.clone(),
                outline: Some(TextOutline::new(OUTLINE, OUTLINE_WIDTH)),
                ..NamedStyle::default()
            },
        ),
        (
            "seams_shadowed",
            NamedStyle {
                text,
                shadow: Some(TextShadow::new(SHADOW, SHADOW_OFFSET)),
                ..NamedStyle::default()
            },
        ),
    ];
    for (name, style) in styles {
        theme.names.text_styles.insert(name.to_owned(), style);
    }
    tree.set_theme(theme);
    for (anchor, offset, caption, style) in [
        (
            Anchor::TopStart,
            Vec2::new(16.0, 16.0),
            "Outline",
            "seams_outlined",
        ),
        (
            Anchor::TopEnd,
            Vec2::new(16.0, 16.0),
            "Shadow",
            "seams_shadowed",
        ),
    ] {
        let mut layout = LayoutStyle::default().anchored(anchor, offset);
        layout.padding = tungsten::core::ui::Edges::all(12.0);
        let root = tree.add_root(UiLayer::Game, layout);
        tree.set_overrides(
            root,
            StyleOverrides {
                background: Some([0, 0, 0, 255]),
                border_width: Some(0.0),
                ..StyleOverrides::default()
            },
        );
        let label = tree.label(root, caption);
        tree.set_text_style(label, Some(style));
    }
}

/// The glow: `engine_white` at the glow's side under `engine_glow`, its
/// colour and falloff in its block.
fn spawn_glow(world: &mut World) {
    let texel = world
        .resource_mut::<AssetRegistry>()
        .intern_sprite(ENGINE_WHITE);
    let mut sprite = Sprite::new(texel);
    sprite.material_id = world.resource::<MaterialRegistry>().get(ENGINE_GLOW);
    world.spawn_with((
        Transform {
            position: GLOW_AT - Vec2::splat(GLOW_SIDE * 0.5),
            rotation: 0.0,
            scale: Vec2::splat(GLOW_SIDE),
        },
        sprite,
        Visibility::default(),
        UniformOverrideBlock::from_slots(
            [GLOW_COLOR, [0.0; 4], [0.0; 4], [0.0; 4]],
            [2.0, 0.0, 0.0, 1.0],
            [0; 4],
        ),
    ));
}

/// The scene's own extract: the segment on the white texel, then the beam.
fn lines(world: &World) -> Vec<tungsten::render::SpriteBatch> {
    let assets = world.resource::<AssetRegistry>();
    let mut batches = Batches::new();
    if let Some(texel) = assets.get_sprite(ENGINE_WHITE) {
        let segment = SpriteInstance::segment(
            SEGMENT_FROM.to_array(),
            SEGMENT_TO.to_array(),
            SEGMENT_WIDTH,
            [0, 255, 255, 255],
            0.0,
        );
        batches.push(texel, segment);
    }
    if let Some(ray) = assets.get_sprite("seams_ray") {
        beam(
            &mut batches,
            ray,
            BEAM_FROM,
            BEAM_TO,
            BEAM_WIDTH,
            0.0,
            [255; 4],
            0.0,
        );
    }
    batches.into_batches()
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
