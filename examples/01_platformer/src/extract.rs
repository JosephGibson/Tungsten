use std::collections::HashMap;

use glam::Vec2;
use tungsten::core::{
    AssetRegistry, CameraState, Entity, FilterMode, InputState, ParallaxLayer, Particle, Sprite,
    Transform, Visibility, World, parallax_world_position,
};
use tungsten::extract_tilemaps;
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

/// M30 parallax backdrop (`D-073`). Emitted before the tilemap, so the layers
/// read sky -> hills -> foliage -> tilemap under `DepthSortMode::CpuStable`
/// batch order. Positions are remapped against the camera here; `Transform` is
/// never mutated.
fn extract_parallax(world: &World, assets: &AssetRegistry) -> Vec<SpriteBatch> {
    let camera_position = world
        .get_resource::<CameraState>()
        .map_or(Vec2::ZERO, |camera| camera.position);

    let mut entries: Vec<(Entity, &Transform, &Sprite, &ParallaxLayer)> = world
        .query3::<Transform, Sprite, ParallaxLayer>()
        .filter(|(entity, _, _, _)| world.get::<Visibility>(*entity).is_some_and(|v| v.visible))
        .collect();
    entries.sort_by(|a, b| {
        a.2.z_order
            .cmp(&b.2.z_order)
            .then_with(|| a.0.id().cmp(&b.0.id()))
    });

    // One batch per `(z_order, atlas, filter)` run: the backdrop layers overlap,
    // so their relative order has to survive batching.
    let mut batches: Vec<SpriteBatch> = Vec::new();
    let mut current: Option<(i32, u32, FilterMode)> = None;
    for (_entity, transform, sprite, layer) in entries {
        let Some(asset) = assets.get_sprite(&sprite.asset_id) else {
            continue;
        };
        let key = (sprite.z_order, asset.atlas.0, asset.filter);
        if current != Some(key) {
            batches.push(SpriteBatch::new(asset.atlas, asset.filter));
            current = Some(key);
        }
        let position =
            parallax_world_position(transform.position, layer.scroll_factor, camera_position);
        let batch = batches.last_mut().expect("batch pushed above");
        batch.instances.push(SpriteInstance {
            position: [position.x, position.y],
            size: [
                asset.width as f32 * transform.scale.x,
                asset.height as f32 * transform.scale.y,
            ],
            rotation: transform.rotation,
            color: sprite.color,
            uv_min: asset.uv.min,
            uv_size: [
                asset.uv.max[0] - asset.uv.min[0],
                asset.uv.max[1] - asset.uv.min[1],
            ],
            z_norm: 0.0,
            _pad: 0.0,
        });
    }
    batches
}

pub(crate) fn extract_sprites(world: &World) -> Vec<SpriteBatch> {
    let Some(assets) = world.get_resource::<AssetRegistry>() else {
        return extract_tilemaps(world);
    };
    // M30: backdrop first, then the tilemap draws the world over it.
    let mut batches = extract_parallax(world, assets);
    batches.extend(extract_tilemaps(world));

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
    batches.extend(particle_batches.into_values());

    // Black hole after tilemap, before player/balls.
    if let Some(hole_asset) = assets.get_sprite("ex10_ball") {
        const BLACK_HOLE_GLOW_DIAMETER: f32 = BLACK_HOLE_VISUAL_DIAMETER * 1.75;
        const BLACK_HOLE_GLOW_COLOR: [u8; 4] = [170, 48, 255, 72];
        const BLACK_HOLE_CORE_COLOR: [u8; 4] = [178, 42, 255, 245];

        let half = BLACK_HOLE_VISUAL_DIAMETER * 0.5;
        let glow_half = BLACK_HOLE_GLOW_DIAMETER * 0.5;
        let uv_min = hole_asset.uv.min;
        let uv_size = [
            hole_asset.uv.max[0] - hole_asset.uv.min[0],
            hole_asset.uv.max[1] - hole_asset.uv.min[1],
        ];
        let hole_positions: Vec<_> = world
            .query::<BlackHole>()
            .filter_map(|(e, _)| world.get::<Position>(e).copied())
            .collect();

        let glow_instances: Vec<SpriteInstance> = hole_positions
            .iter()
            .map(|p| SpriteInstance {
                position: [p.0.x - glow_half, p.0.y - glow_half],
                size: [BLACK_HOLE_GLOW_DIAMETER, BLACK_HOLE_GLOW_DIAMETER],
                rotation: 0.0,
                color: BLACK_HOLE_GLOW_COLOR,
                uv_min,
                uv_size,
                z_norm: 0.0,
                _pad: 0.0,
            })
            .collect();
        if !glow_instances.is_empty() {
            let mut batch = SpriteBatch::new(hole_asset.atlas, hole_asset.filter);
            batch.instances = glow_instances;
            batches.push(batch);
        }

        let core_instances: Vec<SpriteInstance> = hole_positions
            .iter()
            .map(|p| SpriteInstance {
                position: [p.0.x - half, p.0.y - half],
                size: [BLACK_HOLE_VISUAL_DIAMETER, BLACK_HOLE_VISUAL_DIAMETER],
                rotation: 0.0,
                color: BLACK_HOLE_CORE_COLOR,
                uv_min,
                uv_size,
                z_norm: 0.0,
                _pad: 0.0,
            })
            .collect();
        if !core_instances.is_empty() {
            let mut batch = SpriteBatch::new(hole_asset.atlas, hole_asset.filter);
            batch.instances = core_instances;
            batches.push(batch);
        }
    }

    // Player sprite bottom-aligned to physics AABB.
    let lighting_on = world
        .get_resource::<LightingFixture>()
        .is_some_and(|fixture| fixture.mode == LightingFixtureMode::On);
    let mut player_batches: HashMap<String, SpriteBatch> = HashMap::new();
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
        let batch = player_batches.entry(cs.0.clone()).or_insert_with(|| {
            let mut b = SpriteBatch::new(asset.atlas, asset.filter);
            b.material_id = material_id;
            b.uniform_overrides = override_block;
            b.lit = lit;
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
        let sprite_w = asset.width as f32;
        let sprite_h = asset.height as f32;
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
            position: [pos.0.x - sprite_w * 0.5, pos.0.y - sprite_h * 0.5],
            size: [BALL_VISUAL_DIAMETER, BALL_VISUAL_DIAMETER],
            rotation: 0.0,
            color,
            uv_min,
            uv_size,
            z_norm: 0.0,
            _pad: 0.0,
        });
    }
    batches.extend(ball_batches.into_values());

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
            position: [16.0, 84.0],
            bounds: None,
        }));
    }
    sections
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
