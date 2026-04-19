use std::collections::HashMap;

use tungsten::core::{AssetRegistry, InputState, World};
use tungsten::extract_tilemaps;
use tungsten::physics::Position;
use tungsten::render::{SpriteBatch, SpriteInstance, TextSection};

use crate::state::{Ball, CurrentSprite, TextDisplayState, BALL_RADIUS, PLAYER_HALF};

pub(crate) fn extract_sprites(world: &World) -> Vec<SpriteBatch> {
    let mut batches = extract_tilemaps(world);
    let Some(assets) = world.get_resource::<AssetRegistry>() else {
        return batches;
    };

    // Player — sprite frame driven by CurrentSprite / AnimationState.
    // Rendered at 1:1 world-pixel scale (camera zoom handles the screen
    // upscale). Sprite is bottom-aligned to the physics AABB so the player
    // visually stands on surfaces rather than sinking into them.
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
        let batch = player_batches
            .entry(cs.0.clone())
            .or_insert_with(|| SpriteBatch {
                texture: asset.texture,
                filter: asset.filter,
                instances: Vec::new(),
            });
        batch.instances.push(SpriteInstance {
            // Centre horizontally on physics centre; align sprite bottom with
            // physics AABB bottom so the character stands on the ground.
            position: [pos.0.x - sprite_w * 0.5, pos.0.y + PLAYER_HALF.y - sprite_h],
            size: [sprite_w, sprite_h],
            rotation: 0.0,
            color: [255; 4],
        });
    }
    batches.extend(player_batches.into_values());

    // Bouncing balls.
    if let Some(ball_asset) = assets.get_sprite("ex10_ball") {
        let instances: Vec<SpriteInstance> = world
            .query::<Ball>()
            .filter_map(|(e, _)| world.get::<Position>(e).copied())
            .map(|p| SpriteInstance {
                position: [p.0.x - BALL_RADIUS, p.0.y - BALL_RADIUS],
                size: [BALL_RADIUS * 2.0, BALL_RADIUS * 2.0],
                rotation: 0.0,
                color: [255; 4],
            })
            .collect();
        if !instances.is_empty() {
            batches.push(SpriteBatch {
                texture: ball_asset.texture,
                filter: ball_asset.filter,
                instances,
            });
        }
    }

    batches
}

/// Renders `section` with a solid dark outline by drawing the same text at
/// eight pixel offsets in a dark colour first, then the original on top.
/// No engine changes needed — just extra TextSections in draw order.
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
    let disp = world
        .get_resource::<TextDisplayState>()
        .map(|s| {
            (
                s.fps, s.contacts, s.grounded, s.music_on, s.vol_pct, s.zoom_pct,
            )
        })
        .unwrap_or((0, 0, false, false, 50, 100));
    let (fps, contacts, grounded, music_on, vol_pct, zoom_pct) = disp;
    let (cursor_pos, cursor_delta, scroll_lines, scroll_pixels) = world
        .get_resource::<InputState>()
        .map(|input| {
            (
                input.cursor_position(),
                input.cursor_delta(),
                input.scroll_line_delta(),
                input.scroll_pixel_delta(),
            )
        })
        .unwrap_or((None, (0.0, 0.0), (0.0, 0.0), (0.0, 0.0)));
    let cursor_label = cursor_pos
        .map(|(x, y)| format!("{x:.1},{y:.1}"))
        .unwrap_or_else(|| "off-window".to_string());

    let mut sections = Vec::new();

    sections.extend(text_outlined(TextSection {
        content: "Tungsten Platformer".into(),
        font_id: "sans_bold".into(),
        font_size: 36.0,
        line_height: 44.0,
        color: [255, 255, 255, 230],
        position: [16.0, 14.0],
        bounds: None,
    }));

    sections.extend(text_outlined(TextSection {
        content: format!(
            "A/D or ←/→ move  Space/LMB jump  M/RMB music  S/MMB stop  1/2/3 volume\n\
             =/- or wheel zoom  F4 HUD  F9 vsync  F11 fullscreen  Esc exit\n\
             grounded:{:<4} contacts:{:<3} music:{:<4} vol:{}%  zoom:{}%  FPS:{}\n\
             cursor:{}  delta:{:.1},{:.1}  wheel lines:{:.1},{:.1}  pixels:{:.1},{:.1}",
            if grounded { "yes" } else { "no" },
            contacts,
            if music_on { "on" } else { "off" },
            vol_pct,
            zoom_pct,
            fps,
            cursor_label,
            cursor_delta.0,
            cursor_delta.1,
            scroll_lines.0,
            scroll_lines.1,
            scroll_pixels.0,
            scroll_pixels.1,
        ),
        font_id: "mono".into(),
        font_size: 24.0,
        line_height: 32.0,
        color: [200, 220, 255, 210],
        position: [16.0, 70.0],
        bounds: None,
    }));

    sections
}
