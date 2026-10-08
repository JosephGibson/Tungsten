use std::collections::HashMap;

use crate::level_layout::PropDepth;
use crate::state::{AnimatedProp, PlayerPresentation, TILE};
use crate::state::{SMALL_BALL_SCALE, SMALL_BALL_START_SPRITE_ID, SmallBall};
use glam::Vec2;
use tungsten::core::{
    AssetRegistry, CameraState, Entity, FilterMode, InputState, ParallaxLayer, Particle, Sprite,
    Time, Transform, Visibility, World, parallax_world_position,
};
use tungsten::core::{MaterialAssetId, MaterialRegistry, SpriteAsset};
use tungsten::core::{TextAlign, TextLayout};
use tungsten::physics::{Position, PrevPosition};
use tungsten::render::{SpriteBatch, SpriteInstance, TextSection};
use tungsten::{WindowSize, extract_tilemap_layers};

use crate::state::{
    BALL_START_SPRITE_ID, BALL_VISUAL_DIAMETER, BLACK_HOLE_VISUAL_DIAMETER, Ball, BallHue,
    BlackHole, CurrentSprite, LightingFixture, LightingFixtureMode, PLAYER_HALF, Player,
    PlayerMaterial, TextDisplayState,
};
use crate::systems::cursor_to_world;

/// The fraction of a step bodies with history are drawn at this frame:
/// `Time::alpha()` while `Time::interpolate()` is on, `None` while it is
/// off or without `Time` (M41).
pub(crate) fn draw_alpha(world: &World) -> Option<f32> {
    world
        .get_resource::<Time>()
        .filter(|time| time.interpolate())
        .map(Time::alpha)
}

/// Where a body is drawn: `prev + (cur - prev) * alpha` with a
/// `PrevPosition` and an `alpha`, its `Position` otherwise. The game's one
/// lerp, the point `physics_sync` writes to a body's `Transform`; the
/// extract and the effects drawn at a body read it, since they read
/// `Position` where the engine's extract reads `Transform`.
pub(crate) fn lerp_drawn(position: Vec2, prev: Option<&PrevPosition>, alpha: Option<f32>) -> Vec2 {
    match (prev, alpha) {
        (Some(prev), Some(alpha)) => prev.0 + (position - prev.0) * alpha,
        _ => position,
    }
}

/// The scene clock at the instant bodies are drawn: `SceneTime`, which
/// `move_obstacles` advances once a step, less the `(1 - alpha)` of a step
/// the drawn bodies trail the last step by, while `Time::interpolate()` is
/// on; `SceneTime` itself otherwise. The scene's animations read it, so they
/// move in every frame, not only in frames that step (`D-139`'s known issue).
pub(crate) fn drawn_scene_time(world: &World) -> f32 {
    let time = world
        .get_resource::<crate::gameplay::SceneTime>()
        .map_or(0.0, |t| t.0);
    match world
        .get_resource::<Time>()
        .filter(|clock| clock.interpolate())
    {
        Some(clock) => (time - (1.0 - clock.alpha()) * clock.fixed_step()).max(0.0),
        None => time,
    }
}

/// [`lerp_drawn`] for one entity; `None` without a `Position`.
pub(crate) fn drawn_position(world: &World, entity: Entity) -> Option<Vec2> {
    let position = world.get::<Position>(entity)?.0;
    Some(lerp_drawn(
        position,
        world.get::<PrevPosition>(entity),
        draw_alpha(world),
    ))
}

#[allow(clippy::many_single_char_names)] // h/x/r/g/b bindings in HSV-to-RGB math
fn rainbow_rgba(hue: f32) -> [u8; 4] {
    let h = hue.rem_euclid(1.0) * 6.0;
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    // At full saturation and value one channel is 1, one is 0 and `x` ramps
    // between them. The 0.55 lift maps 0 and 1 to themselves, so only `x`
    // needs the `powf`.
    let x = (x.powf(0.55) * 255.0).round().clamp(0.0, 255.0) as u8;
    let (r, g, b) = match h as u32 {
        0 => (255, x, 0),
        1 => (x, 255, 0),
        2 => (0, 255, x),
        3 => (0, x, 255),
        4 => (x, 0, 255),
        _ => (255, 0, x),
    };
    [r, g, b, 255]
}

fn view_bounds(world: &World) -> (Vec2, Vec2) {
    let camera = world
        .get_resource::<CameraState>()
        .copied()
        .unwrap_or_default();
    let window = world
        .get_resource::<WindowSize>()
        .copied()
        .unwrap_or(WindowSize {
            width: 1920,
            height: 1080,
        });
    camera.visible_world_aabb(window.width as f32, window.height as f32)
}

fn instance(asset: &SpriteAsset, position: Vec2, size: Vec2) -> SpriteInstance {
    SpriteInstance {
        position: position.to_array(),
        size: size.to_array(),
        rotation: 0.0,
        color: [255; 4],
        uv_min: asset.uv.min,
        uv_size: [
            asset.uv.max[0] - asset.uv.min[0],
            asset.uv.max[1] - asset.uv.min[1],
        ],
        z_norm: 0.0,
        _pad: 0.0,
    }
}

/// Mirror a sprite about its vertical axis without moving its quad.
fn flip_horizontally(sprite: &mut SpriteInstance) {
    sprite.uv_min[0] += sprite.uv_size[0];
    sprite.uv_size[0] = -sprite.uv_size[0];
}

/// Keep contiguous atlas/filter runs; callers create a fresh list at depth boundaries.
fn push_instance(batches: &mut Vec<SpriteBatch>, asset: &SpriteAsset, sprite: SpriteInstance) {
    push_instance_with_lighting(batches, asset, sprite, asset.lit_atlas.is_some());
}

fn push_instance_with_lighting(
    batches: &mut Vec<SpriteBatch>,
    asset: &SpriteAsset,
    sprite: SpriteInstance,
    lit: bool,
) {
    // Never join a material batch: it would draw this sprite with the glow shader.
    if batches.last().is_none_or(|b| {
        b.texture != asset.atlas
            || b.filter != asset.filter
            || b.lit != lit
            || b.material_id.is_some()
    }) {
        let mut batch = SpriteBatch::new(asset.atlas, asset.filter);
        batch.lit = lit;
        batches.push(batch);
    }
    batches.last_mut().unwrap().instances.push(sprite);
}

/// The example's analytic soft-glow materials (`soft_glow.wgsl`), when registered.
#[derive(Clone, Copy, Default)]
struct GlowMaterials {
    halo: Option<MaterialAssetId>,
    flame: Option<MaterialAssetId>,
}

impl GlowMaterials {
    fn from_world(world: &World) -> Self {
        let registry = world.get_resource::<MaterialRegistry>();
        let get = |name| registry.and_then(|r| r.get(name));
        Self {
            halo: get("ex10_soft_halo"),
            flame: get("ex10_soft_flame"),
        }
    }

    fn for_sprite(self, id: &str) -> Option<MaterialAssetId> {
        match id {
            "ex10_halo" => self.halo,
            "ex10_flame_glow" => self.flame,
            _ => None,
        }
    }
}

/// Glows draw as smooth analytic falloffs through their material; without one
/// (headless tests) they fall back to the dithered glow texture.
fn push_glow(
    batches: &mut Vec<SpriteBatch>,
    asset: &SpriteAsset,
    sprite: SpriteInstance,
    material: Option<MaterialAssetId>,
) {
    let Some(material) = material else {
        push_instance(batches, asset, sprite);
        return;
    };
    if batches
        .last()
        .is_none_or(|b| b.material_id != Some(material) || b.texture != asset.atlas)
    {
        let mut batch = SpriteBatch::new(asset.atlas, asset.filter);
        batch.material_id = Some(material);
        batches.push(batch);
    }
    batches.last_mut().unwrap().instances.push(sprite);
}

fn extract_parallax(world: &World, assets: &AssetRegistry) -> Vec<SpriteBatch> {
    let camera = world
        .get_resource::<CameraState>()
        .copied()
        .unwrap_or_default();
    let (view_min, view_max) = view_bounds(world);
    let overhang = Vec2::splat(2.0 * TILE);
    let mut entries: Vec<(Entity, &Transform, &Sprite, &ParallaxLayer)> = world
        .query::<(Entity, &Transform, &Sprite, &ParallaxLayer)>()
        .filter(|(entity, _, _, _)| world.get::<Visibility>(*entity).is_some_and(|v| v.visible))
        .collect();
    entries.sort_by_key(|(e, _, sprite, _)| (sprite.z_order, e.id()));
    let mut batches = Vec::new();
    for (_, transform, sprite, layer) in entries {
        let Some(asset) = assets.sprite(sprite.asset_id) else {
            continue;
        };
        let name = assets.sprite_name(sprite.asset_id).unwrap_or_default();
        let mut batch = SpriteBatch::new(asset.atlas, asset.filter);
        if name == "ex10_sky" {
            // Fill the actual viewport with the authored sky instead of stretching
            // a tiny portion across the entire level. Overhang absorbs camera shake.
            batch.instances.push(instance(
                asset,
                view_min - overhang,
                view_max - view_min + overhang * 2.0,
            ));
        } else {
            let size = Vec2::new(asset.width as f32, asset.height as f32) * transform.scale;
            let cloud = name.starts_with("ex10_clouds");
            let time = drawn_scene_time(world);
            let drift = if cloud {
                Vec2::new(time * layer.scroll_factor.x * 28.0, 0.0)
            } else {
                Vec2::ZERO
            };
            let remapped = parallax_world_position(
                transform.position + drift,
                layer.scroll_factor,
                camera.position,
            );
            let start = ((view_min.x - overhang.x - remapped.x) / size.x).floor() as i32;
            let end = ((view_max.x + overhang.x - remapped.x) / size.x).ceil() as i32;
            for col in start..end {
                let position = remapped + Vec2::new(col as f32 * size.x, 0.0);
                batch.instances.push(instance(asset, position, size));
                // Extend the opaque strip foot below its art, including vertical
                // travel and resized views. UV samples the last pixel row only.
                let bottom = position.y + size.y;
                if !cloud && bottom < view_max.y + overhang.y {
                    let mut fill = instance(
                        asset,
                        Vec2::new(position.x, bottom),
                        Vec2::new(size.x, view_max.y + overhang.y - bottom),
                    );
                    fill.uv_min[1] = asset.uv.max[1]
                        - (asset.uv.max[1] - asset.uv.min[1]) / asset.height as f32 * 0.5;
                    fill.uv_size[1] = 0.0;
                    batch.instances.push(fill);
                }
            }
        }
        batches.push(batch);
        if name == "ex10_sky" {
            let view = view_max - view_min;
            let center = view_min + view * Vec2::new(0.81, 0.12);
            // Aspect-independent circular disc; the sky itself may stretch.
            let diameter = view.y * 0.135;
            let glows = GlowMaterials::from_world(world);
            for (id, size, color) in [
                ("ex10_halo", diameter * 2.6, [230, 215, 160, 150]),
                ("ex10_flame_glow", diameter * 1.35, [255, 237, 183, 100]),
                ("ex10_moon", diameter, [255; 4]),
            ] {
                if let Some(asset) = assets.get_sprite(id) {
                    let mut moon =
                        instance(asset, center - Vec2::splat(size * 0.5), Vec2::splat(size));
                    moon.color = color;
                    push_glow(&mut batches, asset, moon, glows.for_sprite(id));
                }
            }
        }
    }
    batches
}

fn extract_props(world: &World, assets: &AssetRegistry, depth: PropDepth) -> Vec<SpriteBatch> {
    let (view_min, view_max) = view_bounds(world);
    let mut entries: Vec<_> = world
        .query::<(Entity, &AnimatedProp)>()
        .filter(|(_, p)| p.0 == depth)
        .collect();
    entries.sort_by_key(|(e, _)| e.id());
    let mut batches = Vec::new();
    for (entity, _) in entries {
        let (Some(transform), Some(sprite)) = (
            world.get::<Transform>(entity),
            world.get::<CurrentSprite>(entity),
        ) else {
            continue;
        };
        let Some(asset) = assets.get_sprite(&sprite.0) else {
            continue;
        };
        let size = Vec2::new(asset.width as f32, asset.height as f32) * transform.scale;
        if (transform.position + size).cmplt(view_min).any()
            || transform.position.cmpgt(view_max).any()
        {
            continue;
        }
        push_instance(
            &mut batches,
            asset,
            instance(asset, transform.position, size),
        );
    }
    batches
}

pub(crate) fn extract_sprites(world: &World) -> Vec<SpriteBatch> {
    let Some(assets) = world.get_resource::<AssetRegistry>() else {
        return vec![];
    };
    // M30: backdrop first, then the tilemap draws the world over it.
    let mut batches = extract_parallax(world, assets);
    batches.extend(extract_tilemap_layers(
        world,
        &["background", "decorations"],
    ));
    batches.extend(extract_props(world, assets, PropDepth::Back));
    batches.extend(extract_tilemap_layers(world, &["terrain"]));
    batches.extend(extract_props(world, assets, PropDepth::World));

    batches.extend(extract_obstacles(world, assets));
    batches.extend(extract_vortices(world, assets, false));

    // Particles before black-hole core; custom extract must include them explicitly.
    let mut particle_batches: HashMap<(u32, FilterMode), SpriteBatch> = HashMap::new();
    for (e, _p, t, s) in world.query::<(Entity, &Particle, &Transform, &Sprite)>() {
        let visible = world.get::<Visibility>(e).is_some_and(|v| v.visible);
        if !visible {
            continue;
        }
        let Some(asset) = assets.sprite(s.asset_id) else {
            continue;
        };
        let uv_size = [
            asset.uv.max[0] - asset.uv.min[0],
            asset.uv.max[1] - asset.uv.min[1],
        ];
        let width_world = asset.width as f32 * t.scale.x;
        let height_world = asset.height as f32 * t.scale.y;
        let top_left = t.position - Vec2::new(width_world * 0.5, height_world * 0.5);
        let batch = particle_batches
            .entry((asset.atlas.0, asset.filter))
            .or_insert_with(|| SpriteBatch::new(asset.atlas, asset.filter));
        batch.instances.push(SpriteInstance {
            position: [top_left.x, top_left.y],
            size: [width_world, height_world],
            rotation: t.rotation,
            color: s.color,
            uv_min: asset.uv.min,
            uv_size,
            z_norm: 0.0,
            _pad: 0.0,
        });
    }
    let mut particles: Vec<_> = particle_batches.into_values().collect();
    particles.sort_by_key(|b| b.texture.0);
    batches.extend(particles);

    batches.extend(extract_vortices(world, assets, true));

    // Player sprite bottom-aligned to physics AABB.
    let lighting_on = world
        .get_resource::<LightingFixture>()
        .is_some_and(|fixture| fixture.mode == LightingFixtureMode::On);
    let mut player_batches = Vec::new();
    for (entity, cs) in world.query::<(Entity, &CurrentSprite)>() {
        // A dead player is gone from the screen until the restart.
        if world.get::<Player>(entity).is_none() || crate::death::player_dead(world) {
            continue;
        }
        let Some(pos) = drawn_position(world, entity).map(Position) else {
            continue;
        };
        let Some(asset) = assets.get_sprite(&cs.0) else {
            continue;
        };
        // M30: `squash_stretch_tick_system` drives `Transform.scale`, so the
        // player quad reads it and stays bottom-centered on the physics AABB —
        // a squash flattens onto the ground instead of sinking through it.
        let scale = world
            .get::<Transform>(entity)
            .map_or(Vec2::ONE, |transform| transform.scale);
        let sprite_w = asset.width as f32 * scale.x;
        let sprite_h = asset.height as f32 * scale.y;
        let uv_min = asset.uv.min;
        let uv_size = [
            asset.uv.max[0] - asset.uv.min[0],
            asset.uv.max[1] - asset.uv.min[1],
        ];
        // M29: when the lighting fixture is on, route the player batch to the
        // lit pipeline. Lit wins over the damage-flash material in the M29
        // platformer fixture; the engine warns once per collision elsewhere.
        let lit = lighting_on && asset.lit_atlas.is_some();
        let material_id = if lit {
            None
        } else {
            world.get::<PlayerMaterial>(entity).map(|m| m.material_id)
        };
        let override_block = if lit {
            None
        } else {
            world
                .get::<tungsten::core::UniformOverrideBlock>(entity)
                .copied()
        };
        let mut batch = SpriteBatch::new(asset.atlas, asset.filter);
        batch.material_id = material_id;
        batch.uniform_overrides = override_block;
        batch.lit = lit;
        let facing_left = world
            .get::<PlayerPresentation>(entity)
            .is_some_and(|p| p.facing_left);
        let uv_min = if facing_left {
            [asset.uv.max[0], asset.uv.min[1]]
        } else {
            uv_min
        };
        let uv_size = if facing_left {
            [-uv_size[0], uv_size[1]]
        } else {
            uv_size
        };
        batch.instances.push(SpriteInstance {
            position: [
                pos.0.x - sprite_w * 0.5,
                pos.0.y + PLAYER_HALF.y - 61.0 * scale.y,
            ],
            size: [sprite_w, sprite_h],
            rotation: 0.0,
            color: if world
                .get::<crate::gameplay::Health>(entity)
                .is_some_and(|h| h.immunity > 0.0 && ((h.immunity * 12.0) as u32).is_multiple_of(2))
            {
                [255, 190, 180, 130]
            } else {
                [255; 4]
            },
            uv_min,
            uv_size,
            z_norm: 0.0,
            _pad: 0.0,
        });
        player_batches.push(batch);
    }
    batches.extend(player_batches);

    batches.extend(extract_balls(world, assets, lighting_on));
    batches.extend(extract_bricks(world, assets));
    batches.extend(extract_flames(world, assets));
    batches.extend(extract_fireballs(world, assets));
    batches.extend(extract_ice_beams(world, assets));
    batches.extend(extract_ice_pulses(world, assets));
    batches.extend(extract_tilemap_layers(world, &["foreground"]));

    batches.extend(extract_hearts(world, assets));

    // Cursor sprite last; world point matches click-spawn mapping.
    if let Some(cursor_asset) = assets.get_sprite("ex10_cursor")
        && let Some(world_pos) = world
            .get_resource::<InputState>()
            .and_then(InputState::cursor_position)
            .map(|(x, y)| Vec2::new(x, y))
            .and_then(|cursor| {
                world
                    .get_resource::<CameraState>()
                    .and_then(|cam| cursor_to_world(cursor, cam))
            })
    {
        let zoom = world
            .get_resource::<CameraState>()
            .map_or(1.0, |c| c.zoom)
            .max(f32::EPSILON);
        let sprite_w = 32.0 / zoom;
        let sprite_h = 32.0 / zoom;
        let uv_min = cursor_asset.uv.min;
        let uv_size = [
            cursor_asset.uv.max[0] - cursor_asset.uv.min[0],
            cursor_asset.uv.max[1] - cursor_asset.uv.min[1],
        ];
        let mut batch = SpriteBatch::new(cursor_asset.atlas, cursor_asset.filter);
        batch.instances = vec![SpriteInstance {
            position: [world_pos.x - sprite_w * 0.5, world_pos.y - sprite_h * 0.5],
            size: [sprite_w, sprite_h],
            rotation: 0.0,
            color: [255; 4],
            uv_min,
            uv_size,
            z_norm: 0.0,
            _pad: 0.0,
        }];
        batches.push(batch);
    }

    batches
}

const BALL_SPRITE_SLOTS: usize = 32;

/// Sprite lookups of one ball extract. Balls share a dozen spin-frame IDs, so
/// a slot picked from the ID's length and last byte, checked by comparing the
/// ID, answers nearly every ball without hashing the string. A collision only
/// costs a registry lookup.
struct BallSprites<'w> {
    assets: &'w AssetRegistry,
    slots: [Option<(&'w str, Option<&'w SpriteAsset>)>; BALL_SPRITE_SLOTS],
}

impl<'w> BallSprites<'w> {
    fn get(&mut self, id: &'w str) -> Option<&'w SpriteAsset> {
        let last = id.as_bytes().last().copied().unwrap_or(0);
        let slot = &mut self.slots[(id.len() + usize::from(last)) % BALL_SPRITE_SLOTS];
        match *slot {
            Some((cached, asset)) if cached == id => asset,
            _ => {
                let asset = self.assets.get_sprite(id);
                *slot = Some((id, asset));
                asset
            }
        }
    }
}

/// On-screen balls in query order: one batch per atlas/filter/lit key,
/// batches ordered by texture.
fn extract_balls(world: &World, assets: &AssetRegistry, lighting_on: bool) -> Vec<SpriteBatch> {
    let mut sprites_by_id = BallSprites {
        assets,
        slots: [None; BALL_SPRITE_SLOTS],
    };
    let mut ball_batches: Vec<SpriteBatch> = Vec::new();
    // Batch of the previous ball: a pit has one or two keys.
    let mut current = 0;
    // The camera shows part of the pit at most.
    let (view_min, view_max) = view_bounds(world);
    let alpha = draw_alpha(world);
    let time = drawn_scene_time(world);
    // Both queries walk the same archetypes in the same order, so the zip
    // reads all seven columns with no per-ball lookup.
    let sprites = world.query::<(
        Entity,
        &Ball,
        &Position,
        Option<&SmallBall>,
        Option<&CurrentSprite>,
    )>();
    let tints = world.query::<(
        Entity,
        &Ball,
        &Position,
        Option<&crate::burning::BallBurn>,
        Option<&BallHue>,
        Option<&PrevPosition>,
    )>();
    for ((entity, _, pos, small, sprite), (_, _, _, burn, hue, prev)) in sprites.zip(tints) {
        let small = small.is_some();
        let diameter = BALL_VISUAL_DIAMETER * if small { SMALL_BALL_SCALE } else { 1.0 };
        let top_left = lerp_drawn(pos.0, prev, alpha) - diameter * 0.5;
        if (top_left + diameter).cmplt(view_min).any() || top_left.cmpgt(view_max).any() {
            continue;
        }
        let fallback = if small {
            SMALL_BALL_START_SPRITE_ID
        } else {
            BALL_START_SPRITE_ID
        };
        let sprite_id = sprite.map_or(fallback, |cs| cs.0.as_str());
        let Some(asset) = sprites_by_id
            .get(sprite_id)
            .or_else(|| sprites_by_id.get(fallback))
        else {
            continue;
        };

        let uv_min = asset.uv.min;
        let uv_size = [
            asset.uv.max[0] - asset.uv.min[0],
            asset.uv.max[1] - asset.uv.min[1],
        ];
        let lit = lighting_on && asset.lit_atlas.is_some();
        let same_key =
            |b: &SpriteBatch| b.texture == asset.atlas && b.filter == asset.filter && b.lit == lit;
        if !ball_batches.get(current).is_some_and(same_key) {
            current = ball_batches.iter().position(same_key).unwrap_or_else(|| {
                let mut batch = SpriteBatch::new(asset.atlas, asset.filter);
                batch.lit = lit;
                ball_batches.push(batch);
                ball_batches.len() - 1
            });
        }
        let color = match burn {
            Some(burn) => crate::burning::ember_tint(entity, *burn, time),
            None => hue.map_or([255; 4], |hue| rainbow_rgba(hue.hue)),
        };
        ball_batches[current].instances.push(SpriteInstance {
            position: top_left.to_array(),
            size: [diameter, diameter],
            rotation: 0.0,
            color,
            uv_min,
            uv_size,
            z_norm: 0.0,
            _pad: 0.0,
        });
    }
    // Stable: batches sharing a texture keep the order their keys first appeared in.
    ball_batches.sort_by_key(|b| b.texture.0);
    ball_batches
}

/// An iron brick's quarters and their offsets in tiles.
const BRICK_QUARTERS: [(&str, Vec2); 4] = [
    ("ex10_iron_brick_big_0_0", Vec2::new(0.0, 0.0)),
    ("ex10_iron_brick_big_0_1", Vec2::new(1.0, 0.0)),
    ("ex10_iron_brick_big_1_0", Vec2::new(0.0, 1.0)),
    ("ex10_iron_brick_big_1_1", Vec2::new(1.0, 1.0)),
];
const FROST_QUARTERS: [&str; 4] = [
    "ex10_iron_frost_big_0_0",
    "ex10_iron_frost_big_0_1",
    "ex10_iron_frost_big_1_0",
    "ex10_iron_frost_big_1_1",
];
const SCRAP_SPRITES: [&str; 4] = [
    "ex10_iron_scrap_0",
    "ex10_iron_scrap_1",
    "ex10_iron_scrap_2",
    "ex10_iron_scrap_3",
];
const SCRAP_FROST_SPRITES: [&str; 4] = [
    "ex10_iron_scrap_frost_0",
    "ex10_iron_scrap_frost_1",
    "ex10_iron_scrap_frost_2",
    "ex10_iron_scrap_frost_3",
];

/// The iron bricks: four 64-pixel quarters each, lit like the masonry, so
/// the art keeps the terrain's pixel scale.
fn extract_bricks(world: &World, assets: &AssetRegistry) -> Vec<SpriteBatch> {
    use crate::brick::IronBrick;
    let (view_min, view_max) = view_bounds(world);
    let mut batches = Vec::new();
    for (e, _) in world.query::<(Entity, &IronBrick)>() {
        let Some(center) = drawn_position(world, e) else {
            continue;
        };
        // The art is 128 pixels with the 120-pixel brick centred in it.
        let top_left = center - Vec2::splat(TILE);
        if (top_left + 2.0 * TILE).cmplt(view_min).any() || top_left.cmpgt(view_max).any() {
            continue;
        }
        let frost_amount = crate::ice::freeze_amount(world, e);
        for (quarter, (id, offset)) in BRICK_QUARTERS.into_iter().enumerate() {
            let Some(asset) = assets.get_sprite(id) else {
                continue;
            };
            let mut sprite = instance(asset, top_left + offset * TILE, Vec2::splat(TILE));
            sprite.color = frost_tint(frost_amount);
            push_instance(&mut batches, asset, sprite);
            if frost_amount > 0.0
                && let Some(frost) = assets.get_sprite(FROST_QUARTERS[quarter])
            {
                let mut coating = instance(frost, top_left + offset * TILE, Vec2::splat(TILE));
                coating.color[3] = (255.0 * frost_amount) as u8;
                push_instance_with_lighting(&mut batches, frost, coating, false);
            }
        }
    }
    let time = drawn_scene_time(world);
    for (e, scrap) in world.query::<(Entity, &crate::brick::IronScrap)>() {
        let Some(center) = drawn_position(world, e) else {
            continue;
        };
        let size = Vec2::splat(TILE * 0.5);
        if (center + size).cmplt(view_min).any() || (center - size).cmpgt(view_max).any() {
            continue;
        }
        let Some(asset) = assets.get_sprite(SCRAP_SPRITES[scrap.variant]) else {
            continue;
        };
        let mut sprite = instance(asset, center - size * 0.5, size);
        let rotation = world
            .get::<crate::brick::ScrapTumble>(e)
            .map_or(0.0, |tumble| tumble.rotation(time));
        sprite.rotation = rotation;
        let frost_amount = crate::ice::freeze_amount(world, e);
        sprite.color = frost_tint(frost_amount);
        push_instance(&mut batches, asset, sprite);
        if frost_amount > 0.0
            && let Some(frost) = assets.get_sprite(SCRAP_FROST_SPRITES[scrap.variant])
        {
            let mut coating = instance(frost, center - size * 0.5, size);
            coating.rotation = rotation;
            coating.color[3] = (255.0 * frost_amount) as u8;
            push_instance_with_lighting(&mut batches, frost, coating, false);
        }
    }
    batches
}

fn frost_tint(amount: f32) -> [u8; 4] {
    [
        (255.0 - 155.0 * amount) as u8,
        (255.0 - 60.0 * amount) as u8,
        255,
        255,
    ]
}

/// A flame tongue on each exposed burning ball in view, over a soft glow;
/// `burning::flame_look` varies both per ball. All glows draw first, so a
/// burning crest costs two batches, not two per ball.
fn extract_flames(world: &World, assets: &AssetRegistry) -> Vec<SpriteBatch> {
    use crate::burning::{BallBurn, FLAME_SIZE, FLAME_SPRITE_IDS, flame_look};
    let (view_min, view_max) = view_bounds(world);
    let time = drawn_scene_time(world);
    let flames: Vec<_> = world
        .query::<(Entity, &BallBurn)>()
        .filter_map(|(e, burn)| {
            let look = flame_look(e, *burn, time)?;
            let center = drawn_position(world, e)?;
            let reach = Vec2::splat(FLAME_SIZE);
            ((center + reach).cmpge(view_min).all() && (center - reach).cmple(view_max).all())
                .then_some((center, look))
        })
        .collect();
    let mut batches = Vec::new();
    if let Some(asset) = assets.get_sprite("ex10_flame_glow") {
        let material = GlowMaterials::from_world(world).flame;
        for &(center, look) in &flames {
            let size = Vec2::splat(look.glow_size);
            let mut glow = instance(asset, center - size * 0.5 - Vec2::new(0.0, 4.0), size);
            glow.color = look.glow;
            push_glow(&mut batches, asset, glow, material);
        }
    }
    let frames = FLAME_SPRITE_IDS.map(|id| assets.get_sprite(id));
    for (center, look) in flames {
        let Some(asset) = frames[look.frame] else {
            continue;
        };
        // The tongue's base sits just below the ball's centre, so it wraps the ball's top.
        let origin = center + Vec2::new(-0.5 * FLAME_SIZE, 6.0 - FLAME_SIZE);
        let mut flame = instance(asset, origin, Vec2::splat(FLAME_SIZE));
        if look.mirror {
            flip_horizontally(&mut flame);
        }
        flame.color = look.color;
        // Self-lit: the flames keep their colours and batch together.
        push_instance_with_lighting(&mut batches, asset, flame, false);
    }
    batches
}

/// Pixel hearts stay at a fixed screen size and read current HP without the
/// diagnostic text timer. Empty outlines remain visible after damage.
fn extract_hearts(world: &World, assets: &AssetRegistry) -> Vec<SpriteBatch> {
    let Some((_, health)) = world.query::<(Entity, &crate::gameplay::Health)>().next() else {
        return vec![];
    };
    let camera = world
        .get_resource::<CameraState>()
        .copied()
        .unwrap_or_default();
    let zoom = camera.zoom.max(f32::EPSILON);
    let mut batches = Vec::new();
    for index in 0..3 {
        let id = if index < health.hearts {
            "ex10_heart_full"
        } else {
            "ex10_heart_empty"
        };
        let Some(asset) = assets.get_sprite(id) else {
            continue;
        };
        let screen = Vec2::new(16.0 + index as f32 * 40.0, 86.0);
        if let Some(position) = cursor_to_world(screen, &camera) {
            push_instance(
                &mut batches,
                asset,
                instance(asset, position, Vec2::splat(40.0 / zoom)),
            );
        }
    }
    batches
}

/// Text outline via eight shadow sections plus original.
fn text_outlined(section: TextSection) -> impl Iterator<Item = TextSection> {
    const STROKE: f32 = 2.0;
    const OUTLINE: [u8; 4] = [0, 0, 0, 210];
    let offsets: &[[f32; 2]] = &[
        [-STROKE, 0.0],
        [STROKE, 0.0],
        [0.0, -STROKE],
        [0.0, STROKE],
        [-STROKE, -STROKE],
        [STROKE, -STROKE],
        [-STROKE, STROKE],
        [STROKE, STROKE],
    ];

    let shadows: Vec<TextSection> = offsets
        .iter()
        .map(|&[dx, dy]| TextSection {
            content: section.content.clone(),
            font_id: section.font_id.clone(),
            font_size: section.font_size,
            line_height: section.line_height,
            color: OUTLINE,
            position: [section.position[0] + dx, section.position[1] + dy],
            bounds: section.bounds,
            // The shadow lines up with the text however it is aligned.
            layout: section.layout.clone(),
        })
        .collect();
    shadows.into_iter().chain(std::iter::once(section))
}

pub(crate) fn extract_text(world: &World) -> Vec<TextSection> {
    if crate::death::player_dead(world) {
        return death_title(world);
    }
    let mut sections = Vec::new();
    let width = world
        .get_resource::<WindowSize>()
        .map_or(1920.0, |w| w.width as f32);
    let controls = if width < 900.0 {
        "Move A/D  Jump Space  LMB balls  MMB small balls (5x)\n\
         Black hole RMB  Fireball M4  Ice hold F  Iron R\n\
         L lantern  M music  S stop  +/- zoom  F4 HUD  Esc exit"
    } else {
        "Move A/D or arrows  Space jump/double jump  LMB balls  MMB small balls (5x)  RMB black hole\n\
         Fireball M4  Ice spray hold F  Iron block R  Lantern L\n\
         M music  S stop  1/2/3 volume  +/- or wheel zoom  F4 HUD  F9 vsync  F11 fullscreen  Esc exit"
    };
    let longest = controls.lines().map(str::len).max().unwrap_or(1) as f32;
    let font_size = ((width - 32.0).max(1.0) / (longest * 0.62)).min(20.0);
    sections.extend(text_outlined(TextSection {
        content: controls.into(),
        font_id: "mono".into(),
        font_size,
        line_height: font_size * 1.2,
        color: [200, 220, 255, 210],
        position: [16.0, 14.0],
        bounds: None,
        ..Default::default()
    }));
    sections.extend(death_title(world));
    if let Some(state) = world.get_resource::<TextDisplayState>() {
        sections.extend(text_outlined(TextSection {
            content: format!(
                "FPS {}  Contacts {}  Grounded {}  Music {}  Vol {}%  Zoom {}%",
                state.fps,
                state.contacts,
                if state.grounded { "yes" } else { "no" },
                if state.music_on { "on" } else { "off" },
                state.vol_pct,
                state.zoom_pct
            ),
            font_id: "mono".into(),
            font_size: 20.0,
            line_height: 26.0,
            color: [190, 255, 210, 220],
            position: [150.0, 94.0],
            bounds: None,
            ..Default::default()
        }));
    }
    sections
}

/// The death screen's title and restart prompt, centred over the fade;
/// screen text draws after the post stack, so the fade never covers it.
fn death_title(world: &World) -> Vec<TextSection> {
    use crate::death::DeathScreen;
    let screen = world
        .get_resource::<DeathScreen>()
        .copied()
        .unwrap_or_default();
    let alpha = screen.title_alpha();
    if alpha <= 0.0 {
        return vec![];
    }
    let window = world
        .get_resource::<WindowSize>()
        .copied()
        .unwrap_or(WindowSize {
            width: 1920,
            height: 1080,
        });
    let (width, height) = (window.width as f32, window.height as f32);
    let scale = (width / 1280.0).min(height / 720.0).min(1.35);
    let elapsed = match screen {
        DeathScreen::Dying { elapsed } => elapsed,
        _ => crate::death::DIM_SECS,
    };
    let settle = tungsten::core::Easing::QuadOut.apply(((elapsed - 0.25) / 0.5).clamp(0.0, 1.0));
    let fade = |rgb: [u8; 3], a: f32| [rgb[0], rgb[1], rgb[2], (a * 255.0) as u8];
    let centered = TextLayout::default().with_align(TextAlign::Center);
    let mut sections = vec![
        TextSection {
            content: "YOUR JOURNEY ENDS HERE".into(),
            font_id: "sans".into(),
            font_size: 22.0 * scale,
            line_height: 30.0 * scale,
            color: fade([194, 150, 148], alpha),
            position: [0.0, height * 0.43 - 52.0 * scale],
            bounds: Some([width, 36.0 * scale]),
            layout: centered.clone(),
        },
        TextSection {
            content: "YOU DIED".into(),
            font_id: "sans_bold".into(),
            font_size: 112.0 * scale * (1.0 + 0.22 * (1.0 - settle)),
            line_height: 145.0 * scale,
            color: fade([245, 55, 48], alpha),
            position: [0.0, height * 0.43 - 18.0 * scale * (1.0 - settle)],
            bounds: Some([width, 160.0 * scale]),
            layout: centered.clone(),
        },
    ];
    // The prompt shows once the restart press is taken.
    let prompt = if screen.accepts_restart() || matches!(screen, DeathScreen::Covering { .. }) {
        alpha
    } else {
        0.0
    };
    if prompt > 0.0 {
        sections.push(TextSection {
            content: "Press Enter to restart".into(),
            font_id: "sans".into(),
            font_size: 28.0 * scale,
            line_height: 36.0 * scale,
            color: fade(
                [232, 222, 212],
                prompt * (0.86 + 0.14 * (elapsed * 3.0).sin()),
            ),
            position: [0.0, height * 0.43 + 155.0 * scale],
            bounds: Some([width, 44.0 * scale]),
            layout: centered,
        });
    }
    let mut outlined: Vec<_> = sections.into_iter().flat_map(text_outlined).collect();
    // Outline shadows fade with the title rather than appearing at full opacity.
    for section in &mut outlined {
        if section.color[..3] == [0, 0, 0] {
            section.color[3] = (section.color[3] as f32 * alpha) as u8;
        }
    }
    outlined
}

fn extract_obstacles(world: &World, assets: &AssetRegistry) -> Vec<SpriteBatch> {
    use crate::gameplay::{Explosion, Glow, Hazard, MovingPlatform};
    let (min, max) = view_bounds(world);
    let time = drawn_scene_time(world);
    let mut batches = Vec::new();
    let glows = GlowMaterials::from_world(world);
    // Soft halos complement native point lights and remain visible on scenery
    // without normal maps. Keep them behind hazard silhouettes.
    if let Some(asset) = assets.get_sprite("ex10_halo") {
        for (e, glow) in world.query::<(Entity, &Glow)>() {
            let Some(center) = crate::gameplay::glow_center(world, e, glow.offset) else {
                continue;
            };
            let radius = glow.radius * (1.0 + 0.04 * (time * 4.0 + e.id() as f32).sin());
            if (center + Vec2::splat(radius)).cmplt(min).any()
                || (center - Vec2::splat(radius)).cmpgt(max).any()
            {
                continue;
            }
            let mut sprite = instance(
                asset,
                center - Vec2::splat(radius),
                Vec2::splat(radius * 2.0),
            );
            sprite.color = glow.color;
            push_glow(&mut batches, asset, sprite, glows.halo);
            if (world.get::<Hazard>(e).is_some()
                || world.get::<crate::gameplay::PlayerLantern>(e).is_some())
                && let Some(inner) = assets.get_sprite("ex10_flame_glow")
            {
                let radius = if world.get::<Hazard>(e).is_some() {
                    62.0
                } else {
                    30.0
                };
                let mut sprite = instance(
                    inner,
                    center - Vec2::splat(radius),
                    Vec2::splat(radius * 2.0),
                );
                sprite.color = glow.color;
                push_glow(&mut batches, inner, sprite, glows.flame);
            }
        }
    }
    for (e, hazard) in world.query::<(Entity, &Hazard)>() {
        let (Some(pos), Some(cs)) = (
            drawn_position(world, e).map(Position),
            world.get::<CurrentSprite>(e),
        ) else {
            continue;
        };
        let Some(asset) = assets.get_sprite(&cs.0) else {
            continue;
        };
        if (pos.0 + Vec2::splat(TILE)).cmplt(min).any()
            || (pos.0 - Vec2::splat(TILE)).cmpgt(max).any()
        {
            continue;
        }
        if !hazard.fire {
            let offset = Vec2::new(32.0, 48.0);
            push_instance(
                &mut batches,
                asset,
                instance(asset, pos.0 - offset, Vec2::splat(TILE)),
            );
            continue;
        }
        // Fireballs face their travel and stretch a little with speed.
        let speed = crate::gameplay::motion_velocity(world, e).map_or(0.0, |v| v.x.abs());
        let stretch = (speed / 110.0).min(1.0) * 0.08;
        let size = Vec2::new(TILE * (1.0 + stretch), TILE * (1.0 - stretch * 0.5));
        let mut sprite = instance(asset, pos.0 - size * 0.5, size);
        if crate::gameplay::fireball_faces_left(world, e) {
            flip_horizontally(&mut sprite);
        }
        // Self-lit: its own point light must not wash out the authored ramp.
        push_instance_with_lighting(&mut batches, asset, sprite, false);
    }
    if let Some(asset) = assets.get_sprite("ex10_lift_deck") {
        for (e, platform) in world.query::<(Entity, &MovingPlatform)>() {
            let Some(pos) = drawn_position(world, e).map(Position) else {
                continue;
            };
            let width = platform.half_width * 2.0 / 3.0;
            for col in 0..3 {
                push_instance(
                    &mut batches,
                    asset,
                    instance(
                        asset,
                        pos.0
                            + Vec2::new(
                                -platform.half_width + col as f32 * width,
                                -crate::level_layout::DECK_DEPTH * 0.5,
                            ),
                        Vec2::new(width, TILE),
                    ),
                );
            }
        }
    }
    if let Some(asset) = assets.get_sprite("ex10_shock_ring") {
        for (e, explosion) in world.query::<(Entity, &Explosion)>() {
            let Some(t) = world.get::<Transform>(e) else {
                continue;
            };
            let progress = (explosion.age / 0.45).clamp(0.0, 1.0);
            let fireball = world.has::<crate::fireball::FireballBlast>(e);
            let spread = if fireball {
                1.0 - (1.0 - progress).powi(3)
            } else {
                progress
            };
            let size = 32.0 + spread * (explosion.size - 32.0);
            let mut sprite = instance(
                asset,
                t.position - Vec2::splat(size / 2.0),
                Vec2::splat(size),
            );
            sprite.color = [255, 210, 140, ((1.0 - progress) * 220.0) as u8];
            push_instance(&mut batches, asset, sprite);
            if fireball {
                let size = size * 0.72;
                let mut front = instance(
                    asset,
                    t.position - Vec2::splat(size * 0.5),
                    Vec2::splat(size),
                );
                front.color = [255, 95, 28, ((1.0 - progress).powi(2) * 240.0) as u8];
                push_instance(&mut batches, asset, front);
                if let Some(core) = assets.get_sprite("ex10_flame_glow") {
                    let flash = (1.0 - explosion.age / 0.18).max(0.0);
                    let size = 100.0 + 180.0 * (1.0 - flash);
                    let mut sprite = instance(
                        core,
                        t.position - Vec2::splat(size * 0.5),
                        Vec2::splat(size),
                    );
                    sprite.color = [255, 245, 200, (flash * 255.0) as u8];
                    push_glow(&mut batches, core, sprite, glows.flame);
                }
            }
        }
    }
    batches
}

/// Black holes draw in two passes around the engine particles: the halo, hot
/// disk and spiral arms beneath them, then infalling streaks, the photon ring
/// and the horizon above, so sparks visibly vanish into the core.
fn extract_vortices(
    world: &World,
    assets: &AssetRegistry,
    over_particles: bool,
) -> Vec<SpriteBatch> {
    const D: f32 = BLACK_HOLE_VISUAL_DIAMETER;
    let time = drawn_scene_time(world);
    let mut batches = Vec::new();
    let glows = GlowMaterials::from_world(world);
    let layers: &[(&str, f32, f32, [u8; 4])] = if over_particles {
        &[
            (
                "ex10_photon_ring",
                D * 0.57,
                time * 6.0,
                [255, 235, 215, 255],
            ),
            ("ex10_vortex_core", D * 0.47, -time * 1.3, [255; 4]),
        ]
    } else {
        &[
            ("ex10_halo", D * 2.6, 0.0, [110, 80, 255, 150]),
            (
                "ex10_accretion_disk",
                D * 1.6,
                time * 0.9,
                [255, 125, 150, 150],
            ),
            ("ex10_vortex", D * 1.8, -time * 2.8, [95, 125, 255, 200]),
            ("ex10_vortex", D * 1.2, time * 4.5, [190, 170, 255, 225]),
        ]
    };
    for (e, hole) in world.query::<(Entity, &BlackHole)>() {
        let Some(pos) = drawn_position(world, e).map(Position) else {
            continue;
        };
        let fade = (hole.remaining * 4.0).min(1.0);
        if over_particles && let Some(asset) = assets.get_sprite("ex10_infall_streak") {
            for i in 0..48 {
                let progress = (i as f32 / 48.0 + time * 0.45).fract();
                let radius = (1.0 - progress) * D * 1.05;
                let angle = i as f32 * 2.399_963_2 + time * 3.5 + progress * 5.0;
                let (sin, cos) = angle.sin_cos();
                // Heading along the spiral: orbiting at 5.75 rad/s while falling inward.
                let heading = Vec2::new(-sin, cos) * radius * 5.75 - Vec2::new(cos, sin) * D * 0.47;
                let size = Vec2::splat(22.0 + progress * 18.0);
                let mut sprite = instance(
                    asset,
                    pos.0 + Vec2::new(cos, sin) * radius - size / 2.0,
                    size,
                );
                sprite.rotation = heading.y.atan2(heading.x);
                let warm = progress * progress;
                sprite.color = [
                    (150.0 + 105.0 * warm) as u8,
                    (200.0 - 10.0 * warm) as u8,
                    (255.0 - 145.0 * warm) as u8,
                    (230.0 * fade * (std::f32::consts::PI * progress).sin()) as u8,
                ];
                push_instance(&mut batches, asset, sprite);
            }
        }
        for &(id, diameter, rotation, color) in layers {
            let Some(asset) = assets.get_sprite(id) else {
                continue;
            };
            let size = diameter * (1.0 + 0.045 * (time * 7.0).sin());
            let mut sprite = instance(asset, pos.0 - Vec2::splat(size / 2.0), Vec2::splat(size));
            sprite.rotation = rotation;
            sprite.color = color;
            sprite.color[3] = (sprite.color[3] as f32 * fade) as u8;
            push_glow(&mut batches, asset, sprite, glows.for_sprite(id));
        }
    }
    batches
}

/// Spell missiles: a hot glow, then the fireball frame turned along its velocity.
fn extract_fireballs(world: &World, assets: &AssetRegistry) -> Vec<SpriteBatch> {
    use crate::fireball::{FIREBALL_VISUAL_SIZE, FireballMissile};
    let mut batches = Vec::new();
    let glows = GlowMaterials::from_world(world);
    for (e, missile) in world.query::<(Entity, &FireballMissile)>() {
        let Some(pos) = drawn_position(world, e) else {
            continue;
        };
        if let Some(asset) = assets.get_sprite("ex10_flame_glow") {
            let size = Vec2::splat(FIREBALL_VISUAL_SIZE * 2.4);
            let mut glow = instance(asset, pos - size * 0.5, size);
            glow.color = [255, 140, 50, 200];
            push_glow(&mut batches, asset, glow, glows.flame);
        }
        let Some(asset) = world
            .get::<CurrentSprite>(e)
            .and_then(|cs| assets.get_sprite(&cs.0))
        else {
            continue;
        };
        let size = Vec2::splat(FIREBALL_VISUAL_SIZE);
        let mut sprite = instance(asset, pos - size * 0.5, size);
        // The art faces +x with its drips below; leftward shots mirror so the
        // drips stay on the underside, then turn the mirrored nose onto the velocity.
        let v = missile.velocity;
        sprite.rotation = if v.x < 0.0 {
            flip_horizontally(&mut sprite);
            (-v.y).atan2(-v.x)
        } else {
            v.y.atan2(v.x)
        };
        push_instance_with_lighting(&mut batches, asset, sprite, false);
    }
    batches
}

/// Soft widening wisps back the snow-crystal and mist particles, with no rigid core.
fn extract_ice_beams(world: &World, assets: &AssetRegistry) -> Vec<SpriteBatch> {
    let mut batches = Vec::new();
    let glows = GlowMaterials::from_world(world);
    let Some(asset) = assets.get_sprite("ex10_flame_glow") else {
        return batches;
    };
    for (_, beam) in world.query::<(Entity, &crate::ice::IceBeam)>() {
        let travel = beam.end - beam.start;
        let length = travel.length().min(beam.age * 800.0 + 24.0);
        if length <= 0.01 {
            continue;
        }
        let direction = travel.normalize();
        let across = Vec2::new(-direction.y, direction.x);
        for i in 0..6 {
            let progress = (i as f32 + 0.5) / 6.0;
            let distance = length * progress;
            let flutter = (beam.age * 18.0 - i as f32 * 1.7).sin();
            let center = beam.start + direction * distance + across * flutter * progress * 9.0;
            let width = 18.0 + distance * 0.45;
            let size = Vec2::new((length / 6.0 * 1.8).max(32.0), width);
            let mut sprite = instance(asset, center - size * 0.5, size);
            sprite.rotation = travel.y.atan2(travel.x);
            sprite.color = [
                100,
                200,
                255,
                (100.0 - progress * 40.0 + flutter * 10.0) as u8,
            ];
            push_glow(&mut batches, asset, sprite, glows.flame);
        }
    }
    batches
}

/// A short icy ring separates the moment of freezing from the burst of fracture.
fn extract_ice_pulses(world: &World, assets: &AssetRegistry) -> Vec<SpriteBatch> {
    let mut batches = Vec::new();
    let glows = GlowMaterials::from_world(world);
    let (view_min, view_max) = view_bounds(world);
    for (_, pulse) in world.query::<(Entity, &crate::ice::IcePulse)>() {
        let progress = (pulse.age / pulse.duration()).clamp(0.0, 1.0);
        let reach = Vec2::splat(pulse.size * 1.3);
        if (pulse.at + reach).cmplt(view_min).any() || (pulse.at - reach).cmpgt(view_max).any() {
            continue;
        }
        let left = 1.0 - progress;
        if let Some(asset) = assets.get_sprite("ex10_flame_glow") {
            let size = Vec2::splat(pulse.size * (1.5 + progress * 0.8));
            let mut glow = instance(asset, pulse.at - size * 0.5, size);
            glow.color = [160, 230, 255, (150.0 * left * left) as u8];
            push_glow(&mut batches, asset, glow, glows.flame);
        }
        if let Some(asset) = assets.get_sprite("ex10_ice_ring") {
            let growth = if pulse.shatter {
                1.0 + progress * 1.5
            } else {
                1.3 + progress * 0.5
            };
            let size = Vec2::splat(pulse.size * growth);
            let mut ring = instance(asset, pulse.at - size * 0.5, size);
            ring.color = [190, 245, 255, (210.0 * left * left) as u8];
            ring.rotation = pulse.age * if pulse.shatter { -1.5 } else { 0.5 };
            push_instance_with_lighting(&mut batches, asset, ring, false);
        }
    }
    batches
}

#[cfg(test)]
mod tests {
    use super::rainbow_rgba;

    #[test]
    fn rainbow_rgba_keeps_primary_hues_fully_saturated() {
        assert_eq!(rainbow_rgba(0.0), [255, 0, 0, 255]);
        assert_eq!(rainbow_rgba(1.0 / 3.0), [0, 255, 0, 255]);
        assert_eq!(rainbow_rgba(2.0 / 3.0), [0, 0, 255, 255]);
    }
}
