//! The UI primitive pipeline (M46, w01 §6): the instance a paint list's
//! rectangles and images upload as, the draw items that interleave primitive
//! batches with text batches in paint order, and the overlay draw that sets
//! each item's scissor. Item building is GPU-free (`build_items`) so the
//! batching has tests without a device. The pipeline draws in the overlay
//! pass after the acquire split (`D-145`), onto a single-sample target with
//! no depth, with straight-alpha blending, as text does.

use std::ops::Range;

use bytemuck::{Pod, Zeroable};
use tungsten_core::assets::{TextureHandle, UvRect};
use tungsten_core::ui::{ClipRect, PaintCommand, PaintList, Rect};
use wgpu::util::DeviceExt;

use crate::sprite::SpritePipeline;

/// Unit-square two-triangle vertex; the instance places and sizes it.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct Vertex {
    position: [f32; 2],
}

impl Vertex {
    const ATTRIBS: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0 => Float32x2];

    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

const UNIT_QUAD: &[Vertex] = &[
    Vertex {
        position: [0.0, 0.0],
    },
    Vertex {
        position: [1.0, 0.0],
    },
    Vertex {
        position: [1.0, 1.0],
    },
    Vertex {
        position: [0.0, 0.0],
    },
    Vertex {
        position: [1.0, 1.0],
    },
    Vertex {
        position: [0.0, 1.0],
    },
];

/// One rounded rectangle or textured rectangle, 80 bytes. `fill`, `border`,
/// `shadow_color` and `gradient_color` are sRGB, decoded in the shader. The
/// shadow and gradient fields are reserved and zero in M46 (Q13).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct UiInstance {
    /// Top-left corner in physical pixels.
    pub position: [f32; 2],
    /// Size in physical pixels.
    pub size: [f32; 2],
    /// Page UV at the top-left corner.
    pub uv_min: [f32; 2],
    /// Page UV span across the box.
    pub uv_size: [f32; 2],
    /// Fill colour, or the tint of a textured instance.
    pub fill: [u8; 4],
    /// Border colour.
    pub border: [u8; 4],
    /// Border width in pixels; zero for none.
    pub border_width: f32,
    /// Corner radius in pixels.
    pub corner_radius: f32,
    /// Reserved: shadow offset.
    pub shadow_offset: [f32; 2],
    /// Reserved: shadow blur.
    pub shadow_blur: f32,
    /// Reserved: shadow colour.
    pub shadow_color: [u8; 4],
    /// Reserved: the second gradient stop.
    pub gradient_color: [u8; 4],
    /// Reserved: gradient direction.
    pub gradient_dir: [f32; 2],
    /// Bit 0: the fill is the page sample times `fill`.
    pub flags: u32,
}

impl UiInstance {
    /// `flags` bit: sample the bound page.
    pub const FLAG_TEXTURED: u32 = 1;

    const ATTRIBS: [wgpu::VertexAttribute; 14] = wgpu::vertex_attr_array![
        1 => Float32x2,
        2 => Float32x2,
        3 => Float32x2,
        4 => Float32x2,
        5 => Unorm8x4,
        6 => Unorm8x4,
        7 => Float32,
        8 => Float32,
        9 => Float32x2,
        10 => Float32,
        11 => Unorm8x4,
        12 => Unorm8x4,
        13 => Float32x2,
        14 => Uint32,
    ];

    /// The per-instance vertex buffer layout.
    #[must_use]
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<UiInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }

    /// An untextured rounded rectangle.
    #[must_use]
    pub fn rect(
        rect: Rect,
        fill: [u8; 4],
        border: [u8; 4],
        border_width: f32,
        corner_radius: f32,
    ) -> Self {
        Self {
            position: rect.min.to_array(),
            size: rect.size().to_array(),
            uv_min: [0.0, 0.0],
            uv_size: [0.0, 0.0],
            fill,
            border,
            border_width,
            corner_radius,
            shadow_offset: [0.0, 0.0],
            shadow_blur: 0.0,
            shadow_color: [0, 0, 0, 0],
            gradient_color: [0, 0, 0, 0],
            gradient_dir: [0.0, 0.0],
            flags: 0,
        }
    }

    /// A textured rectangle: `uv` of the bound page times `tint`.
    #[must_use]
    pub fn image(rect: Rect, uv: UvRect, tint: [u8; 4]) -> Self {
        Self {
            uv_min: uv.min,
            uv_size: [uv.max[0] - uv.min[0], uv.max[1] - uv.min[1]],
            flags: Self::FLAG_TEXTURED,
            ..Self::rect(rect, tint, [0, 0, 0, 0], 0.0, 0.0)
        }
    }
}

/// One draw of the overlay, in paint order. Every item carries the scissor
/// the draw sets before it: a primitive batch its clip, a text batch the
/// viewport, since glyphon clips through `TextBounds` and leaves the pass's
/// scissor as it finds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DrawItem {
    /// A run of instances sharing one page (`None` for the white page) and
    /// one clip.
    Primitives {
        texture: Option<TextureHandle>,
        scissor: ClipRect,
        instances: Range<u32>,
    },
    /// The text batch at this index of the paint list's partition, drawn
    /// where its range closes.
    Text { batch: usize, scissor: ClipRect },
}

/// The scissor an item sets on an attachment of `size`: its clip intersected
/// with the attachment, as `(x, y, width, height)`; `None` when the
/// intersection is empty, so the item is skipped. The acquire can shrink the
/// surface before the resize reaches the world, and the paint list was
/// extracted before it.
pub(crate) fn attachment_scissor(clip: ClipRect, size: (u32, u32)) -> Option<(u32, u32, u32, u32)> {
    let attachment = ClipRect {
        x: 0,
        y: 0,
        width: i32::try_from(size.0).unwrap_or(i32::MAX),
        height: i32::try_from(size.1).unwrap_or(i32::MAX),
    };
    let inner = clip.intersect(attachment);
    if inner.is_empty() {
        return None;
    }
    Some((
        inner.x as u32,
        inner.y as u32,
        inner.width as u32,
        inner.height as u32,
    ))
}

/// Builds the overlay's draw items from `list` and its instances into
/// `instances`: one primitive item per run of commands sharing a page and a
/// clip, a text item where each batch of [`PaintList::text_batches`] closes,
/// after the non-text commands inside its range, in paint order. Text
/// commands upload nothing; their batch draws them.
pub(crate) fn build_items(list: &PaintList, instances: &mut Vec<UiInstance>) -> Vec<DrawItem> {
    instances.clear();
    let viewport = list.clips.first().copied().unwrap_or_default();
    let batches = list.text_batches();
    let mut next_batch = 0;
    let mut items = Vec::new();
    let mut open: Option<(Option<TextureHandle>, u32, u32)> = None;
    let close = |open: &mut Option<(Option<TextureHandle>, u32, u32)>,
                 items: &mut Vec<DrawItem>,
                 end: u32| {
        if let Some((texture, clip, start)) = open.take() {
            items.push(DrawItem::Primitives {
                texture,
                scissor: list.clips.get(clip as usize).copied().unwrap_or_default(),
                instances: start..end,
            });
        }
    };
    for (i, command) in list.commands.iter().enumerate() {
        push_primitive(list, command, &mut open, &mut items, instances, &close);
        if batches
            .get(next_batch)
            .is_some_and(|batch| batch.end == i + 1)
        {
            close(&mut open, &mut items, instances.len() as u32);
            items.push(DrawItem::Text {
                batch: next_batch,
                scissor: viewport,
            });
            next_batch += 1;
        }
    }
    close(&mut open, &mut items, instances.len() as u32);
    items
}

/// Uploads `command`'s instance, when it has one, into the open primitive
/// run or a new one when its page or clip differ.
fn push_primitive(
    list: &PaintList,
    command: &PaintCommand,
    open: &mut Option<(Option<TextureHandle>, u32, u32)>,
    items: &mut Vec<DrawItem>,
    instances: &mut Vec<UiInstance>,
    close: &impl Fn(&mut Option<(Option<TextureHandle>, u32, u32)>, &mut Vec<DrawItem>, u32),
) {
    {
        let _ = list;
        let (key, instance) = match *command {
            PaintCommand::Rect {
                rect,
                fill,
                border,
                border_width,
                corner_radius,
                clip,
            } => (
                (None, clip),
                UiInstance::rect(rect, fill, border, border_width, corner_radius),
            ),
            PaintCommand::Image {
                rect,
                texture,
                uv,
                tint,
                clip,
                ..
            } => ((Some(texture), clip), UiInstance::image(rect, uv, tint)),
            // Text draws through its batch; a command kind this crate does
            // not know uploads nothing (`PaintCommand` is non-exhaustive).
            _ => return,
        };
        if open.is_none_or(|(texture, clip, _)| (texture, clip) != key) {
            close(open, items, instances.len() as u32);
            *open = Some((key.0, key.1, instances.len() as u32));
        }
        instances.push(instance);
    }
}

/// Instances the buffer holds before it first grows.
const INSTANCE_CAPACITY: usize = 256;

/// The UI primitive pipeline and the frame's uploaded items.
pub struct UiPipeline {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    viewport_buffer: wgpu::Buffer,
    viewport_bind_group: wgpu::BindGroup,
    /// The attachment size the projection was written for.
    viewport_written: Option<(u32, u32)>,
    /// A 1×1 opaque white page for untextured instances.
    white_bind_group: wgpu::BindGroup,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
    instances: Vec<UiInstance>,
    items: Vec<DrawItem>,
}

impl UiPipeline {
    /// The pipeline for the overlay target of `format`, sharing the sprite
    /// pipeline's `texture_layout` (group 1) so atlas pages bind as they are.
    #[must_use]
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        texture_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ui_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("ui.wgsl").into()),
        });

        let viewport_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ui_viewport_uniform"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let viewport_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ui_viewport_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let viewport_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ui_viewport_bg"),
            layout: &viewport_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: viewport_buffer.as_entire_binding(),
            }],
        });

        let white_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ui_white_page"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &white_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255, 255, 255, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let white_view = white_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let white_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("ui_white_sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let white_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ui_white_bg"),
            layout: texture_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&white_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&white_sampler),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ui_pipeline_layout"),
            bind_group_layouts: &[Some(&viewport_layout), Some(texture_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ui_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(Vertex::desc()), Some(UiInstance::desc())],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            },
            // The overlay target is single-sample with no depth.
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ui_vertex_buffer"),
            contents: bytemuck::cast_slice(UNIT_QUAD),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let instance_buffer = create_instance_buffer(device, INSTANCE_CAPACITY);

        Self {
            pipeline,
            vertex_buffer,
            viewport_buffer,
            viewport_bind_group,
            viewport_written: None,
            white_bind_group,
            instance_buffer,
            instance_capacity: INSTANCE_CAPACITY,
            instances: Vec::new(),
            items: Vec::new(),
        }
    }

    /// Builds the frame's items from `list` and uploads its instances for an
    /// attachment of `width` × `height`. An empty list uploads nothing; the
    /// projection is written when the size changed.
    pub(crate) fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        list: &PaintList,
        width: u32,
        height: u32,
    ) {
        self.items = build_items(list, &mut self.instances);
        if self.instances.is_empty() {
            return;
        }
        if self.viewport_written != Some((width, height)) {
            let projection = glam::camera::rh::proj::directx::orthographic(
                0.0,
                width as f32,
                height as f32,
                0.0,
                -1.0,
                1.0,
            );
            let matrix: &[f32; 16] = projection.as_ref();
            queue.write_buffer(&self.viewport_buffer, 0, bytemuck::cast_slice(matrix));
            self.viewport_written = Some((width, height));
        }
        if self.instances.len() > self.instance_capacity {
            let capacity = self.instances.len().next_power_of_two();
            self.instance_buffer = create_instance_buffer(device, capacity);
            self.instance_capacity = capacity;
        }
        queue.write_buffer(
            &self.instance_buffer,
            0,
            bytemuck::cast_slice(&self.instances),
        );
    }

    /// The items `upload` built, in paint order.
    pub(crate) fn items(&self) -> &[DrawItem] {
        &self.items
    }

    /// Draws a primitive item: its scissor clamped to `attachment`, the page
    /// bind group from `sprites` (the white page for an untextured run or a
    /// page `sprites` does not hold), then its instances. An item whose
    /// scissor misses the attachment draws nothing.
    pub(crate) fn draw_primitives(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        item: &DrawItem,
        sprites: &SpritePipeline,
        attachment: (u32, u32),
    ) {
        let DrawItem::Primitives {
            texture,
            scissor,
            instances,
        } = item
        else {
            return;
        };
        let Some((x, y, width, height)) = attachment_scissor(*scissor, attachment) else {
            return;
        };
        pass.set_scissor_rect(x, y, width, height);
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.viewport_bind_group, &[]);
        let page = texture
            .and_then(|handle| sprites.texture_bind_group(handle))
            .unwrap_or(&self.white_bind_group);
        pass.set_bind_group(1, page, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        pass.draw(0..6, instances.clone());
    }
}

fn create_instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ui_instance_buffer"),
        size: (capacity * std::mem::size_of::<UiInstance>()) as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

#[cfg(test)]
#[path = "tests/ui.rs"]
mod tests;
