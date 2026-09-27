use std::collections::HashMap;

use crate::level_layout::PropDepth;
use crate::state::{AnimatedProp, PlayerPresentation, TILE};
use glam::Vec2;
use tungsten::WindowSize;
use tungsten::core::assets::LayerKind;
use tungsten::core::{
    AssetRegistry, CameraState, Entity, FilterMode, InputState, ParallaxLayer, Particle, Sprite,
    Transform, Visibility, World, parallax_world_position,
};
use tungsten::core::{SpriteAsset, TilemapInstance, TilemapRegistry};
use tungsten::physics::Position;
use tungsten::render::{SpriteBatch, SpriteInstance, TextSection};

use crate::state::{
    BALL_START_SPRITE_ID, BALL_VISUAL_DIAMETER, BLACK_HOLE_VISUAL_DIAMETER, Ball, BallHue,
    BlackHole, CurrentSprite, LightingFixture, LightingFixtureMode, PLAYER_HALF, Player,
    PlayerMaterial, TextDisplayState,
};
use crate::systems::cursor_to_world;

#[allow(clippy::many_single_char_names)] // h/c/x/r/g/b bindings in HSV-to-RGB math
fn rainbow_rgba(hue: f32) -> [u8; 4] {
    let h = hue.rem_euclid(1.0) * 6.0;
    let c = 1.0;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let saturated = |v: f32| -> u8 {
        let lifted = v.powf(0.55);
        (lifted * 255.0).round().clamp(0.0, 255.0) as u8
    };
    [saturated(r), saturated(g), saturated(b), 255]
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

/// Keep contiguous atlas/filter runs; callers create a fresh list at depth boundaries.
fn push_instance(batches: &mut Vec<SpriteBatch>, asset: &SpriteAsset, sprite: SpriteInstance) {
    if batches.last().is_none_or(|b| {
        b.texture != asset.atlas || b.filter != asset.filter || b.lit != asset.lit_atlas.is_some()
    }) {
        let mut batch = SpriteBatch::new(asset.atlas, asset.filter);
        batch.lit = asset.lit_atlas.is_some();
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
        .query3::<Transform, Sprite, ParallaxLayer>()
        .filter(|(entity, _, _, _)| world.get::<Visibility>(*entity).is_some_and(|v| v.visible))
        .collect();
    entries.sort_by_key(|(e, _, sprite, _)| (sprite.z_order, e.id()));
    let mut batches = Vec::new();
    for (_, transform, sprite, layer) in entries {
        let Some(asset) = assets.get_sprite(&sprite.asset_id) else {
            continue;
        };
        let mut batch = SpriteBatch::new(asset.atlas, asset.filter);
        if sprite.asset_id == "ex10_sky" {
            // Fill the actual viewport with the authored sky instead of stretching
            // a tiny portion across the entire level. Overhang absorbs camera shake.
            batch.instances.push(instance(
                asset,
                view_min - overhang,
                view_max - view_min + overhang * 2.0,
            ));
        } else {
            let size = Vec2::new(asset.width as f32, asset.height as f32) * transform.scale;
            let cloud = sprite.asset_id.starts_with("ex10_clouds");
            let time = world
                .get_resource::<crate::gameplay::SceneTime>()
                .map_or(0.0, |t| t.0);
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
        if sprite.asset_id == "ex10_sky" {
            let view = view_max - view_min;
            let center = view_min + view * Vec2::new(0.81, 0.12);
            // Aspect-independent circular disc; the sky itself may stretch.
            let diameter = view.y * 0.135;
            for (id, size, color) in [
                ("ex10_halo", diameter * 2.6, [230, 215, 160, 150]),
                ("ex10_flame_glow", diameter * 1.35, [255, 237, 183, 100]),
                ("ex10_moon", diameter, [255; 4]),
            ] {
                if let Some(asset) = assets.get_sprite(id) {
                    let mut moon =
                        instance(asset, center - Vec2::splat(size * 0.5), Vec2::splat(size));
                    moon.color = color;
                    push_instance(&mut batches, asset, moon);
                }
            }
        }
    }
    batches
}

/// Public tile data is enough for example-local stage selection and culling.
pub(crate) fn extract_tile_layers(world: &World, names: &[&str]) -> Vec<SpriteBatch> {
    let (Some(assets), Some(tilemaps)) = (
        world.get_resource::<AssetRegistry>(),
        world.get_resource::<TilemapRegistry>(),
    ) else {
        return vec![];
    };
    let (view_min, view_max) = view_bounds(world);
    let mut result = Vec::new();
    let mut maps: Vec<_> = world.query::<TilemapInstance>().collect();
    maps.sort_by_key(|(entity, _)| entity.id());
    for (_, map) in maps {
        let Some(data) = tilemaps.get(&map.id) else {
            continue;
        };
        let tile = Vec2::new(data.tile_width as f32, data.tile_height as f32);
        let start = ((view_min - map.origin) / tile)
            .floor()
            .max(Vec2::ZERO)
            .as_uvec2();
        let end = ((view_max - map.origin) / tile)
            .ceil()
            .max(Vec2::ZERO)
            .as_uvec2()
            .min(glam::UVec2::new(data.width, data.height));
        for layer in &data.layers {
            if layer.kind != LayerKind::Render || !names.contains(&layer.name.as_str()) {
                continue;
            }
            let mut runs = Vec::new();
            for row in start.y..end.y {
                for col in start.x..end.x {
                    let index = layer.tiles[(row * data.width + col) as usize];
                    if index < 0 {
                        continue;
                    }
                    let Some(id) = data.tileset.get(index as usize) else {
                        continue;
                    };
                    let Some(asset) = assets.get_sprite(id) else {
                        continue;
                    };
                    push_instance(
                        &mut runs,
                        asset,
                        instance(
                            asset,
                            map.origin + Vec2::new(col as f32, row as f32) * tile,
                            tile,
                        ),
                    );
                }
            }
            result.extend(runs);
        }
    }
    result
}

fn extract_props(world: &World, assets: &AssetRegistry, depth: PropDepth) -> Vec<SpriteBatch> {
    let (view_min, view_max) = view_bounds(world);
    let mut entries: Vec<_> = world
        .query::<AnimatedProp>()
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
    batches.extend(extract_tile_layers(world, &["background", "decorations"]));
    batches.extend(extract_props(world, assets, PropDepth::Back));
    batches.extend(extract_tile_layers(world, &["terrain"]));
    batches.extend(extract_props(world, assets, PropDepth::World));

    batches.extend(extract_obstacles(world, assets));

    // Particles before black-hole core; custom extract must include them explicitly.
    let mut particle_batches: HashMap<(u32, FilterMode), SpriteBatch> = HashMap::new();
    for (e, _p, t, s) in world.query3::<Particle, Transform, Sprite>() {
        let visible = world.get::<Visibility>(e).is_some_and(|v| v.visible);
        if !visible {
            continue;
        }
        let Some(asset) = assets.get_sprite(&s.asset_id) else {
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

    batches.extend(extract_vortices(world, assets));

    // Player sprite bottom-aligned to physics AABB.
    let lighting_on = world
        .get_resource::<LightingFixture>()
        .is_some_and(|fixture| fixture.mode == LightingFixtureMode::On);
    let mut player_batches = Vec::new();
    for (entity, cs) in world.query::<CurrentSprite>() {
        if world.get::<Player>(entity).is_none() {
            continue;
        }
        let Some(pos) = world.get::<Position>(entity).copied() else {
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

    let mut ball_batches: HashMap<(u32, FilterMode, bool), SpriteBatch> = HashMap::new();
    for (entity, _) in world.query::<Ball>() {
        let Some(pos) = world.get::<Position>(entity).copied() else {
            continue;
        };
        let sprite_id = world
            .get::<CurrentSprite>(entity)
            .map_or(BALL_START_SPRITE_ID, |cs| cs.0.as_str());
        let Some(asset) = assets
            .get_sprite(sprite_id)
            .or_else(|| assets.get_sprite(BALL_START_SPRITE_ID))
        else {
            continue;
        };

        let uv_min = asset.uv.min;
        let uv_size = [
            asset.uv.max[0] - asset.uv.min[0],
            asset.uv.max[1] - asset.uv.min[1],
        ];
        let lit = lighting_on && asset.lit_atlas.is_some();
        let batch = ball_batches
            .entry((asset.atlas.0, asset.filter, lit))
            .or_insert_with(|| {
                let mut b = SpriteBatch::new(asset.atlas, asset.filter);
                b.lit = lit;
                b
            });
        let color = world
            .get::<BallHue>(entity)
            .map_or([255, 0, 255, 255], |hue| rainbow_rgba(hue.hue));
        batch.instances.push(SpriteInstance {
            position: [
                pos.0.x - BALL_VISUAL_DIAMETER * 0.5,
                pos.0.y - BALL_VISUAL_DIAMETER * 0.5,
            ],
            size: [BALL_VISUAL_DIAMETER, BALL_VISUAL_DIAMETER],
            rotation: 0.0,
            color,
            uv_min,
            uv_size,
            z_norm: 0.0,
            _pad: 0.0,
        });
    }
    let mut balls: Vec<_> = ball_batches.into_values().collect();
    balls.sort_by_key(|b| b.texture.0);
    batches.extend(balls);
    batches.extend(extract_tile_layers(world, &["foreground"]));

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

/// Pixel hearts stay at a fixed screen size and read current HP without the
/// diagnostic text timer. Empty outlines remain visible after damage.
fn extract_hearts(world: &World, assets: &AssetRegistry) -> Vec<SpriteBatch> {
    let Some((_, health)) = world.query::<crate::gameplay::Health>().next() else {
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
        })
        .collect();
    shadows.into_iter().chain(std::iter::once(section))
}

pub(crate) fn extract_text(world: &World) -> Vec<TextSection> {
    let mut sections = Vec::new();
    sections.extend(text_outlined(TextSection {
        content: "A/D or ←/→ move  Space jump / double jump  LMB hold spawn ball  RMB black hole  M music  S/MMB stop  1/2/3 volume\n\
                  =/- or wheel zoom (35–300%)  L lantern  Avoid spikes and fire  F4 HUD  F9 vsync  F11 fullscreen  Esc exit"
            .into(),
        font_id: "mono".into(),
        font_size: 24.0,
        line_height: 32.0,
        color: [200, 220, 255, 210],
        position: [16.0, 14.0],
        bounds: None,
    }));
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
        }));
    }
    sections
}

fn extract_obstacles(world: &World, assets: &AssetRegistry) -> Vec<SpriteBatch> {
    use crate::gameplay::{Explosion, Glow, Hazard, MovingPlatform, SceneTime};
    let (min, max) = view_bounds(world);
    let time = world.get_resource::<SceneTime>().map_or(0.0, |t| t.0);
    let mut batches = Vec::new();
    // Soft sprite halos complement native point lights and remain visible on
    // scenery without normal maps. Keep them behind hazard silhouettes.
    if let Some(asset) = assets.get_sprite("ex10_halo") {
        for (e, glow) in world.query::<Glow>() {
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
            push_instance(&mut batches, asset, sprite);
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
                push_instance(&mut batches, inner, sprite);
            }
        }
    }
    for (e, hazard) in world.query::<Hazard>() {
        let (Some(pos), Some(cs)) = (world.get::<Position>(e), world.get::<CurrentSprite>(e))
        else {
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
        let offset = if hazard.fire {
            Vec2::splat(32.0)
        } else {
            Vec2::new(32.0, 48.0)
        };
        push_instance(
            &mut batches,
            asset,
            instance(asset, pos.0 - offset, Vec2::splat(TILE)),
        );
    }
    if let Some(asset) = assets.get_sprite("ex10_lift_deck") {
        for (e, platform) in world.query::<MovingPlatform>() {
            let Some(pos) = world.get::<Position>(e) else {
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
        for (e, explosion) in world.query::<Explosion>() {
            let Some(t) = world.get::<Transform>(e) else {
                continue;
            };
            let progress = explosion.age / 0.45;
            let size = 32.0 + progress * 108.0;
            let mut sprite = instance(
                asset,
                t.position - Vec2::splat(size / 2.0),
                Vec2::splat(size),
            );
            sprite.color = [255, 210, 140, ((1.0 - progress) * 220.0) as u8];
            push_instance(&mut batches, asset, sprite);
        }
    }
    batches
}

fn extract_vortices(world: &World, assets: &AssetRegistry) -> Vec<SpriteBatch> {
    let time = world
        .get_resource::<crate::gameplay::SceneTime>()
        .map_or(0.0, |t| t.0);
    let mut batches = Vec::new();
    for (e, hole) in world.query::<BlackHole>() {
        let Some(pos) = world.get::<Position>(e) else {
            continue;
        };
        let fade = (hole.remaining * 4.0).min(1.0);
        for (id, diameter, rotation, color) in [
            (
                "ex10_halo",
                BLACK_HOLE_VISUAL_DIAMETER * 2.5,
                0.0,
                [135, 95, 255, 230],
            ),
            (
                "ex10_vortex",
                BLACK_HOLE_VISUAL_DIAMETER * 1.8,
                -time * 2.8,
                [140, 150, 255, 150],
            ),
            (
                "ex10_vortex",
                BLACK_HOLE_VISUAL_DIAMETER * 1.2,
                time * 4.5,
                [220, 200, 255, 240],
            ),
            (
                "ex10_vortex_core",
                BLACK_HOLE_VISUAL_DIAMETER * 0.65,
                -time * 1.3,
                [255; 4],
            ),
        ] {
            let Some(asset) = assets.get_sprite(id) else {
                continue;
            };
            let size = diameter * (1.0 + 0.045 * (time * 7.0).sin());
            let mut sprite = instance(asset, pos.0 - Vec2::splat(size / 2.0), Vec2::splat(size));
            sprite.rotation = rotation;
            sprite.color = color;
            sprite.color[3] = (sprite.color[3] as f32 * fade) as u8;
            push_instance(&mut batches, asset, sprite);
        }
        if let Some(asset) = assets.get_sprite("ex10_spark") {
            for i in 0..32 {
                let progress = (i as f32 / 32.0 + time * 0.45).fract();
                let radius = (1.0 - progress) * BLACK_HOLE_VISUAL_DIAMETER;
                let angle = i as f32 * 2.399_963_2 + time * 3.5 + progress * 5.0;
                let center = pos.0 + Vec2::new(angle.cos(), angle.sin()) * radius;
                let size = 6.0 + progress * 7.0;
                let mut sprite =
                    instance(asset, center - Vec2::splat(size / 2.0), Vec2::splat(size));
                sprite.color = [
                    160,
                    210,
                    255,
                    (220.0 * fade * (std::f32::consts::PI * progress).sin()) as u8,
                ];
                push_instance(&mut batches, asset, sprite);
            }
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
