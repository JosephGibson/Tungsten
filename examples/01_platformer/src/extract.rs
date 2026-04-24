use std::collections::HashMap;

use glam::Vec2;
use tungsten::core::{
    AssetRegistry, CameraState, FilterMode, InputState, Particle, Sprite, Transform, Visibility,
    World,
};
use tungsten::extract_tilemaps;
use tungsten::physics::Position;
use tungsten::render::{SpriteBatch, SpriteInstance, TextSection};

use crate::state::{
    Ball, BlackHole, CurrentSprite, PlayerMaterial, BALL_RADIUS, BLACK_HOLE_VISUAL_DIAMETER,
    PLAYER_HALF,
};
use crate::systems::cursor_to_world;

pub(crate) fn extract_sprites(world: &World) -> Vec<SpriteBatch> {
    let mut batches = extract_tilemaps(world);
    let Some(assets) = world.get_resource::<AssetRegistry>() else {
        return batches;
    };

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
        let batch = particle_batches
            .entry((asset.atlas.0, asset.filter))
            .or_insert_with(|| SpriteBatch::new(asset.atlas, asset.filter));
        batch.instances.push(SpriteInstance {
            position: [t.position.x, t.position.y],
            size: [width_world, height_world],
            rotation: t.rotation,
            color: s.color,
            uv_min: asset.uv.min,
            uv_size,
            z_norm: 0.0,
            _pad: 0.0,
        });
    }
    batches.extend(particle_batches.into_values());

    // Black hole after tilemap, before player/balls.
    if let Some(hole_asset) = assets.get_sprite("ex10_ball") {
        let half = BLACK_HOLE_VISUAL_DIAMETER * 0.5;
        let uv_min = hole_asset.uv.min;
        let uv_size = [
            hole_asset.uv.max[0] - hole_asset.uv.min[0],
            hole_asset.uv.max[1] - hole_asset.uv.min[1],
        ];
        let instances: Vec<SpriteInstance> = world
            .query::<BlackHole>()
            .filter_map(|(e, _)| world.get::<Position>(e).copied())
            .map(|p| SpriteInstance {
                position: [p.0.x - half, p.0.y - half],
                size: [BLACK_HOLE_VISUAL_DIAMETER, BLACK_HOLE_VISUAL_DIAMETER],
                rotation: 0.0,
                color: [115, 20, 191, 230],
                uv_min,
                uv_size,
                z_norm: 0.0,
                _pad: 0.0,
            })
            .collect();
        if !instances.is_empty() {
            let mut batch = SpriteBatch::new(hole_asset.atlas, hole_asset.filter);
            batch.instances = instances;
            batches.push(batch);
        }
    }

    // Player sprite bottom-aligned to physics AABB.
    let mut player_batches: HashMap<String, SpriteBatch> = HashMap::new();
    for (entity, cs) in world.query::<CurrentSprite>() {
        let Some(pos) = world.get::<Position>(entity).copied() else {
            continue;
        };
        let Some(asset) = assets.get_sprite(&cs.0) else {
            continue;
        };
        let sprite_w = asset.width as f32;
        let sprite_h = asset.height as f32;
        let uv_min = asset.uv.min;
        let uv_size = [
            asset.uv.max[0] - asset.uv.min[0],
            asset.uv.max[1] - asset.uv.min[1],
        ];
        let material_id = world.get::<PlayerMaterial>(entity).map(|m| m.material_id);
        let override_block = world
            .get::<tungsten::core::UniformOverrideBlock>(entity)
            .copied();
        let batch = player_batches.entry(cs.0.clone()).or_insert_with(|| {
            let mut b = SpriteBatch::new(asset.atlas, asset.filter);
            b.material_id = material_id;
            b.uniform_overrides = override_block;
            b
        });
        batch.instances.push(SpriteInstance {
            position: [pos.0.x - sprite_w * 0.5, pos.0.y + PLAYER_HALF.y - sprite_h],
            size: [sprite_w, sprite_h],
            rotation: 0.0,
            color: [255; 4],
            uv_min,
            uv_size,
            z_norm: 0.0,
            _pad: 0.0,
        });
    }
    batches.extend(player_batches.into_values());

    if let Some(ball_asset) = assets.get_sprite("ex10_ball") {
        let uv_min = ball_asset.uv.min;
        let uv_size = [
            ball_asset.uv.max[0] - ball_asset.uv.min[0],
            ball_asset.uv.max[1] - ball_asset.uv.min[1],
        ];
        let instances: Vec<SpriteInstance> = world
            .query::<Ball>()
            .filter_map(|(e, _)| world.get::<Position>(e).copied())
            .map(|p| SpriteInstance {
                position: [p.0.x - BALL_RADIUS, p.0.y - BALL_RADIUS],
                size: [BALL_RADIUS * 2.0, BALL_RADIUS * 2.0],
                rotation: 0.0,
                color: [255; 4],
                uv_min,
                uv_size,
                z_norm: 0.0,
                _pad: 0.0,
            })
            .collect();
        if !instances.is_empty() {
            let mut batch = SpriteBatch::new(ball_asset.atlas, ball_asset.filter);
            batch.instances = instances;
            batches.push(batch);
        }
    }

    // Cursor sprite last; world point matches click-spawn mapping.
    if let Some(cursor_asset) = assets.get_sprite("ex10_cursor") {
        if let Some(world_pos) = world
            .get_resource::<InputState>()
            .and_then(InputState::cursor_position)
            .map(|(x, y)| Vec2::new(x, y))
            .and_then(|cursor| {
                world
                    .get_resource::<CameraState>()
                    .and_then(|cam| cursor_to_world(cursor, cam))
            })
        {
            let sprite_w = cursor_asset.width as f32;
            let sprite_h = cursor_asset.height as f32;
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

pub(crate) fn extract_text(_world: &World) -> Vec<TextSection> {
    // Controls only; telemetry lives in HUD/inspector.
    let mut sections = Vec::new();
    sections.extend(text_outlined(TextSection {
        content: "A/D or ←/→ move  Space jump  LMB hold spawn ball  RMB black hole  M music  S/MMB stop  1/2/3 volume\n\
                  =/- or wheel zoom  F4 HUD  F9 vsync  F11 fullscreen  Esc exit"
            .into(),
        font_id: "mono".into(),
        font_size: 24.0,
        line_height: 32.0,
        color: [200, 220, 255, 210],
        position: [16.0, 14.0],
        bounds: None,
    }));
    sections
}
