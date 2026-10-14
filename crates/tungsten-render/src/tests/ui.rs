use std::mem::{align_of, offset_of, size_of};

use glam::Vec2;
use tungsten_core::text::TextNodeId;

use super::*;

const WHITE: [u8; 4] = [255, 255, 255, 255];

fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect::from_pos_size(Vec2::new(x, y), Vec2::new(width, height))
}

fn clip(x: i32, y: i32, width: i32, height: i32) -> ClipRect {
    ClipRect {
        x,
        y,
        width,
        height,
    }
}

fn rect_command(rect: Rect, clip: u32) -> PaintCommand {
    PaintCommand::Rect {
        rect,
        fill: [10, 20, 30, 255],
        border: [1, 2, 3, 128],
        border_width: 1.5,
        corner_radius: 4.0,
        clip,
    }
}

fn image_command(rect: Rect, texture: u32, clip: u32) -> PaintCommand {
    PaintCommand::Image {
        rect,
        texture: TextureHandle(texture),
        uv: UvRect {
            min: [0.25, 0.5],
            max: [0.75, 1.0],
        },
        filter: tungsten_core::assets::FilterMode::Nearest,
        tint: [255, 0, 0, 255],
        clip,
    }
}

fn text_command(rect: Rect, clip: u32) -> PaintCommand {
    PaintCommand::Text {
        node: TextNodeId::new(0, 0),
        rect,
        color: WHITE,
        clip,
    }
}

fn list(clips: Vec<ClipRect>, commands: Vec<PaintCommand>) -> PaintList {
    PaintList {
        scale: 1.0,
        clips,
        commands,
    }
}

#[test]
fn the_instance_is_80_bytes_of_packed_4_byte_fields() {
    assert_eq!(size_of::<UiInstance>(), 80);
    assert_eq!(align_of::<UiInstance>(), 4);
    let desc = UiInstance::desc();
    assert_eq!(desc.array_stride, 80);
    assert_eq!(desc.step_mode, wgpu::VertexStepMode::Instance);
    let expected: [(u64, wgpu::VertexFormat); 14] = [
        (
            offset_of!(UiInstance, position) as u64,
            wgpu::VertexFormat::Float32x2,
        ),
        (
            offset_of!(UiInstance, size) as u64,
            wgpu::VertexFormat::Float32x2,
        ),
        (
            offset_of!(UiInstance, uv_min) as u64,
            wgpu::VertexFormat::Float32x2,
        ),
        (
            offset_of!(UiInstance, uv_size) as u64,
            wgpu::VertexFormat::Float32x2,
        ),
        (
            offset_of!(UiInstance, fill) as u64,
            wgpu::VertexFormat::Unorm8x4,
        ),
        (
            offset_of!(UiInstance, border) as u64,
            wgpu::VertexFormat::Unorm8x4,
        ),
        (
            offset_of!(UiInstance, border_width) as u64,
            wgpu::VertexFormat::Float32,
        ),
        (
            offset_of!(UiInstance, corner_radius) as u64,
            wgpu::VertexFormat::Float32,
        ),
        (
            offset_of!(UiInstance, shadow_offset) as u64,
            wgpu::VertexFormat::Float32x2,
        ),
        (
            offset_of!(UiInstance, shadow_blur) as u64,
            wgpu::VertexFormat::Float32,
        ),
        (
            offset_of!(UiInstance, shadow_color) as u64,
            wgpu::VertexFormat::Unorm8x4,
        ),
        (
            offset_of!(UiInstance, gradient_color) as u64,
            wgpu::VertexFormat::Unorm8x4,
        ),
        (
            offset_of!(UiInstance, gradient_dir) as u64,
            wgpu::VertexFormat::Float32x2,
        ),
        (
            offset_of!(UiInstance, flags) as u64,
            wgpu::VertexFormat::Uint32,
        ),
    ];
    assert_eq!(desc.attributes.len(), expected.len());
    for (i, (attribute, (offset, format))) in desc.attributes.iter().zip(expected).enumerate() {
        assert_eq!(attribute.shader_location, i as u32 + 1, "attribute {i}");
        assert_eq!(attribute.offset, offset, "attribute {i}");
        assert_eq!(attribute.format, format, "attribute {i}");
    }
    assert_eq!(offset_of!(UiInstance, flags) + 4, 80, "no trailing padding");
}

#[test]
fn constructors_fill_the_instance_from_a_command() {
    let instance = UiInstance::rect(
        rect(1.0, 2.0, 30.0, 40.0),
        [1, 2, 3, 4],
        [5, 6, 7, 8],
        1.5,
        4.0,
    );
    assert_eq!(instance.position, [1.0, 2.0]);
    assert_eq!(instance.size, [30.0, 40.0]);
    assert_eq!(instance.fill, [1, 2, 3, 4]);
    assert_eq!(instance.border, [5, 6, 7, 8]);
    assert_eq!(instance.border_width, 1.5);
    assert_eq!(instance.corner_radius, 4.0);
    assert_eq!(instance.flags, 0);
    assert_eq!(instance.uv_size, [0.0, 0.0]);
    assert_eq!(instance.shadow_blur, 0.0);
    assert_eq!(instance.gradient_color, [0, 0, 0, 0]);

    let uv = UvRect {
        min: [0.25, 0.5],
        max: [0.75, 1.0],
    };
    let image = UiInstance::image(rect(1.0, 2.0, 30.0, 40.0), uv, [255, 0, 0, 128]);
    assert_eq!(image.position, [1.0, 2.0]);
    assert_eq!(image.uv_min, [0.25, 0.5]);
    assert_eq!(image.uv_size, [0.5, 0.5]);
    assert_eq!(image.fill, [255, 0, 0, 128]);
    assert_eq!(image.border, [0, 0, 0, 0]);
    assert_eq!(image.border_width, 0.0);
    assert_eq!(image.flags, UiInstance::FLAG_TEXTURED);
}

#[test]
fn items_split_at_page_and_clip_changes_and_keep_paint_order() {
    let clips = vec![clip(0, 0, 800, 600), clip(10, 10, 100, 100)];
    let list = list(
        clips.clone(),
        vec![
            rect_command(rect(0.0, 0.0, 10.0, 10.0), 0),
            rect_command(rect(10.0, 0.0, 10.0, 10.0), 0),
            image_command(rect(20.0, 0.0, 10.0, 10.0), 3, 0),
            image_command(rect(30.0, 0.0, 10.0, 10.0), 3, 1),
            rect_command(rect(40.0, 0.0, 10.0, 10.0), 1),
            image_command(rect(50.0, 0.0, 10.0, 10.0), 4, 1),
            image_command(rect(60.0, 0.0, 10.0, 10.0), 4, 1),
        ],
    );
    let mut instances = Vec::new();
    let items = build_items(&list, &mut instances);
    assert_eq!(
        items,
        [
            DrawItem::Primitives {
                texture: None,
                scissor: clips[0],
                instances: 0..2,
            },
            DrawItem::Primitives {
                texture: Some(TextureHandle(3)),
                scissor: clips[0],
                instances: 2..3,
            },
            DrawItem::Primitives {
                texture: Some(TextureHandle(3)),
                scissor: clips[1],
                instances: 3..4,
            },
            DrawItem::Primitives {
                texture: None,
                scissor: clips[1],
                instances: 4..5,
            },
            DrawItem::Primitives {
                texture: Some(TextureHandle(4)),
                scissor: clips[1],
                instances: 5..7,
            },
        ]
    );
    assert_eq!(instances.len(), 7);
    let positions: Vec<f32> = instances.iter().map(|i| i.position[0]).collect();
    assert_eq!(positions, [0.0, 10.0, 20.0, 30.0, 40.0, 50.0, 60.0]);
    assert_eq!(instances[0].fill, [10, 20, 30, 255]);
    assert_eq!(instances[2].flags, UiInstance::FLAG_TEXTURED);
    assert_eq!(instances[2].uv_min, [0.25, 0.5]);
}

#[test]
fn a_text_item_sits_where_its_batch_closes_with_the_viewport_scissor() {
    let clips = vec![clip(0, 0, 800, 600), clip(10, 10, 100, 100)];
    let list = list(
        clips.clone(),
        vec![
            rect_command(rect(10.0, 10.0, 100.0, 100.0), 1),
            text_command(rect(10.0, 10.0, 50.0, 20.0), 1),
            rect_command(rect(10.0, 50.0, 100.0, 50.0), 1),
            text_command(rect(20.0, 70.0, 50.0, 20.0), 1),
            rect_command(rect(0.0, 0.0, 300.0, 300.0), 0),
            text_command(rect(5.0, 5.0, 50.0, 20.0), 0),
        ],
    );
    let mut instances = Vec::new();
    let items = build_items(&list, &mut instances);
    assert_eq!(
        items,
        [
            DrawItem::Primitives {
                texture: None,
                scissor: clips[1],
                instances: 0..2,
            },
            DrawItem::Text {
                batch: 0,
                scissor: clips[0],
            },
            DrawItem::Primitives {
                texture: None,
                scissor: clips[0],
                instances: 2..3,
            },
            DrawItem::Text {
                batch: 1,
                scissor: clips[0],
            },
        ],
        "the rect inside batch 0's range draws before the batch; text never inherits a primitive clip"
    );
    assert_eq!(instances.len(), 3, "text commands upload nothing");
}

#[test]
fn adjacent_text_commands_of_one_batch_make_one_item() {
    let list = list(
        vec![clip(0, 0, 800, 600)],
        vec![
            text_command(rect(0.0, 0.0, 50.0, 20.0), 0),
            text_command(rect(0.0, 30.0, 50.0, 20.0), 0),
            rect_command(rect(0.0, 100.0, 5.0, 5.0), 0),
            text_command(rect(0.0, 60.0, 50.0, 20.0), 0),
        ],
    );
    let mut instances = Vec::new();
    let items = build_items(&list, &mut instances);
    assert_eq!(
        items,
        [
            DrawItem::Primitives {
                texture: None,
                scissor: clip(0, 0, 800, 600),
                instances: 0..1,
            },
            DrawItem::Text {
                batch: 0,
                scissor: clip(0, 0, 800, 600),
            },
        ],
        "the batch draws where it closes; the rect inside its range draws before it"
    );
}

#[test]
fn an_empty_list_yields_no_items_and_no_instances() {
    let mut instances = vec![UiInstance::rect(
        rect(0.0, 0.0, 1.0, 1.0),
        WHITE,
        WHITE,
        0.0,
        0.0,
    )];
    let items = build_items(&PaintList::default(), &mut instances);
    assert!(items.is_empty());
    assert!(
        instances.is_empty(),
        "upload clears the previous frame's instances"
    );
}

#[test]
fn scissors_are_clamped_to_the_attachment_and_empty_ones_skipped() {
    assert_eq!(
        attachment_scissor(clip(0, 0, 800, 600), (640, 480)),
        Some((0, 0, 640, 480)),
        "the acquire shrank the surface under the paint list"
    );
    assert_eq!(attachment_scissor(clip(700, 0, 200, 100), (640, 480)), None);
    assert_eq!(
        attachment_scissor(clip(-10, -10, 50, 50), (640, 480)),
        Some((0, 0, 40, 40))
    );
    assert_eq!(attachment_scissor(clip(10, 10, 0, 5), (640, 480)), None);
    assert_eq!(
        attachment_scissor(clip(10, 10, 20, 20), (640, 480)),
        Some((10, 10, 20, 20))
    );
    assert_eq!(attachment_scissor(clip(0, 0, 10, 10), (0, 0)), None);
}

#[test]
fn items_past_the_buffer_limit_are_cut() {
    let list = list(
        vec![clip(0, 0, 800, 600)],
        vec![
            rect_command(rect(0.0, 0.0, 10.0, 10.0), 0),
            rect_command(rect(10.0, 0.0, 10.0, 10.0), 0),
            text_command(rect(0.0, 20.0, 50.0, 10.0), 0),
            rect_command(rect(0.0, 40.0, 10.0, 10.0), 0),
            image_command(rect(0.0, 60.0, 10.0, 10.0), 3, 0),
        ],
    );
    let mut instances = Vec::new();
    let mut items = build_items(&list, &mut instances);
    assert_eq!(instances.len(), 4);
    assert!(cap_items(&mut items, &mut instances, 3));
    assert_eq!(instances.len(), 3);
    let ranges: Vec<_> = items
        .iter()
        .filter_map(|item| match item {
            DrawItem::Primitives { instances, .. } => Some(instances.clone()),
            DrawItem::Text { .. } => None,
        })
        .collect();
    assert_eq!(ranges, [0..2, 2..3]);
    assert!(
        items
            .iter()
            .any(|item| matches!(item, DrawItem::Text { .. }))
    );
    assert!(!cap_items(&mut items, &mut instances, 3));
}
