//! Draw and upload counts (redo 7.3). Every queue write, buffer created with
//! contents and draw the engine records goes through these wrappers, so
//! [`Renderer::frame_counters`](crate::Renderer::frame_counters) reports the
//! engine's own work each frame. Glyphon uploads and draws inside its
//! `prepare` and `render`, out of their reach, so the text overlay counts
//! its `render` calls instead.

use std::ops::Range;
use std::sync::atomic::{AtomicU64, Ordering};

use wgpu::util::DeviceExt as _;

/// One frame's counts: what the engine recorded between the end of the
/// frame before and the end of this one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameCounters {
    /// Draw calls the engine recorded; glyphon's own are not among them.
    pub draw_calls: u64,
    /// Bytes the engine wrote to buffers and textures or created buffers
    /// with; glyphon's own are not among them.
    pub upload_bytes: u64,
    /// Glyphon `render` calls, each a text draw glyphon records itself.
    pub text_renders: u64,
}

/// One owner's running counts since the renderer last took them. Atomic and
/// boxed, so the pipelines that hold one keep their auto traits.
#[derive(Debug, Default)]
pub(crate) struct Counts {
    draws: AtomicU64,
    bytes: AtomicU64,
    text_renders: AtomicU64,
}

impl Counts {
    pub(crate) fn boxed() -> Box<Self> {
        Box::default()
    }

    fn add_bytes(&self, bytes: u64) {
        self.bytes.fetch_add(bytes, Ordering::Relaxed);
    }

    fn add_draw(&self) {
        self.draws.fetch_add(1, Ordering::Relaxed);
    }

    /// Counts one glyphon `render` call.
    pub(crate) fn text_render(&self) {
        self.text_renders.fetch_add(1, Ordering::Relaxed);
    }

    /// Adds the counts to `total` and starts them over.
    pub(crate) fn take_into(&self, total: &mut FrameCounters) {
        total.draw_calls += self.draws.swap(0, Ordering::Relaxed);
        total.upload_bytes += self.bytes.swap(0, Ordering::Relaxed);
        total.text_renders += self.text_renders.swap(0, Ordering::Relaxed);
    }
}

/// `queue.write_buffer`, counted.
pub(crate) fn write_buffer(
    counts: &Counts,
    queue: &wgpu::Queue,
    buffer: &wgpu::Buffer,
    offset: wgpu::BufferAddress,
    data: &[u8],
) {
    counts.add_bytes(data.len() as u64);
    queue.write_buffer(buffer, offset, data);
}

/// `queue.write_buffer_with`, counted at its size when it gives a view.
pub(crate) fn write_buffer_with(
    counts: &Counts,
    queue: &wgpu::Queue,
    buffer: &wgpu::Buffer,
    offset: wgpu::BufferAddress,
    size: wgpu::BufferSize,
) -> Option<wgpu::QueueWriteBufferView> {
    let view = queue.write_buffer_with(buffer, offset, size)?;
    counts.add_bytes(size.get());
    Some(view)
}

/// `queue.write_texture`, counted at the bytes it reads.
pub(crate) fn write_texture(
    counts: &Counts,
    queue: &wgpu::Queue,
    texture: wgpu::TexelCopyTextureInfo<'_>,
    data: &[u8],
    layout: wgpu::TexelCopyBufferLayout,
    size: wgpu::Extent3d,
) {
    counts.add_bytes(data.len().saturating_sub(layout.offset as usize) as u64);
    queue.write_texture(texture, data, layout, size);
}

/// `device.create_buffer_init`, counted at its contents.
pub(crate) fn create_buffer_init(
    counts: &Counts,
    device: &wgpu::Device,
    desc: &wgpu::util::BufferInitDescriptor<'_>,
) -> wgpu::Buffer {
    counts.add_bytes(desc.contents.len() as u64);
    device.create_buffer_init(desc)
}

/// `pass.draw`, counted.
pub(crate) fn draw(
    counts: &Counts,
    pass: &mut wgpu::RenderPass<'_>,
    vertices: Range<u32>,
    instances: Range<u32>,
) {
    counts.add_draw();
    pass.draw(vertices, instances);
}

/// `pass.draw_indexed`, counted.
pub(crate) fn draw_indexed(
    counts: &Counts,
    pass: &mut wgpu::RenderPass<'_>,
    indices: Range<u32>,
    base_vertex: i32,
    instances: Range<u32>,
) {
    counts.add_draw();
    pass.draw_indexed(indices, base_vertex, instances);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_add_up_and_start_over_when_taken() {
        let counts = Counts::boxed();
        counts.add_bytes(64);
        counts.add_draw();
        counts.add_draw();
        counts.text_render();
        let mut total = FrameCounters {
            draw_calls: 1,
            ..FrameCounters::default()
        };
        counts.take_into(&mut total);
        assert_eq!(
            total,
            FrameCounters {
                draw_calls: 3,
                upload_bytes: 64,
                text_renders: 1,
            }
        );
        let mut again = FrameCounters::default();
        counts.take_into(&mut again);
        assert_eq!(again, FrameCounters::default());
    }
}
