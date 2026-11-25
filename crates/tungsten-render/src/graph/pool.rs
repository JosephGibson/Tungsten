//! The textures a frame's graph borrows.
//!
//! The pool keeps a texture while frames use it and a while after, so a
//! frame of the same shape finds its textures, and every bind group over
//! them, where the last one left them. Each texture has an identity that is
//! never given again: whatever holds a view of a pooled texture keys on it.

use std::time::Duration;

use super::{Class, Spec};

/// A pooled texture goes when no frame has used it for this many frames
/// and for [`KEEP_TIME`]: a screen transition's extra target outlives the
/// gaps between two of them, and a capture's does not outlive the capture
/// by long.
pub(crate) const KEEP_FRAMES: u64 = 240;

/// The time [`KEEP_FRAMES`] takes at 60 frames a second. The frames alone
/// would be a fraction of a second at an uncapped frame rate, and every
/// transition would allocate its target again.
pub(crate) const KEEP_TIME: Duration = Duration::from_secs(4);

struct Entry<T> {
    id: u64,
    spec: Spec,
    texture: T,
    /// The last frame that used it, and the time that frame began.
    used: u64,
    used_at: Duration,
}

/// Textures of kind `T` by [`Spec`]. `T` is the GPU objects, generic so the
/// pool's rules are tested without a device.
pub(crate) struct Pool<T> {
    entries: Vec<Entry<T>>,
    next_id: u64,
    frame: u64,
    /// When the last frame ended, on the caller's clock.
    now: Duration,
}

impl<T> Default for Pool<T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            next_id: 1,
            frame: 0,
            now: Duration::ZERO,
        }
    }
}

impl<T> Pool<T> {
    /// A texture for each of `slots`, no texture twice: one the pool holds
    /// with that spec, the oldest first, else a new one from `make`. The
    /// same slots against the same pool give the same textures, so a frame
    /// that compiles again finds its own.
    pub(crate) fn bind(
        &mut self,
        slots: &[Spec],
        mut make: impl FnMut(&Spec, u64) -> T,
    ) -> Vec<u64> {
        let mut bound: Vec<u64> = Vec::with_capacity(slots.len());
        for spec in slots {
            let held = self
                .entries
                .iter()
                .find(|entry| entry.spec == *spec && !bound.contains(&entry.id))
                .map(|entry| entry.id);
            let id = held.unwrap_or_else(|| {
                let id = self.next_id;
                self.next_id += 1;
                self.entries.push(Entry {
                    id,
                    spec: *spec,
                    texture: make(spec, id),
                    used: self.frame,
                    used_at: self.now,
                });
                id
            });
            bound.push(id);
        }
        self.touch(&bound);
        bound
    }

    /// Marks `bound` as used by the current frame.
    pub(crate) fn touch(&mut self, bound: &[u64]) {
        let (frame, now) = (self.frame, self.now);
        for entry in &mut self.entries {
            if bound.contains(&entry.id) {
                entry.used = frame;
                entry.used_at = now;
            }
        }
    }

    /// The texture with identity `id`.
    ///
    /// # Panics
    ///
    /// When the pool no longer holds it: a frame binds its textures before
    /// it reads them, and the pool frees none a frame has just used.
    pub(crate) fn get(&self, id: u64) -> &T {
        self.entries
            .iter()
            .find(|entry| entry.id == id)
            .map(|entry| &entry.texture)
            .expect("a bound texture is in the pool")
    }

    /// Ends a frame at `now`, a time on any clock that only moves forward:
    /// frees every texture unused for [`KEEP_FRAMES`] frames and for
    /// [`KEEP_TIME`], and returns their identities, so their views can be
    /// dropped too.
    pub(crate) fn end_frame(&mut self, now: Duration) -> Vec<u64> {
        self.frame += 1;
        self.now = now;
        let frame = self.frame;
        self.drain(|entry| {
            frame - entry.used > KEEP_FRAMES && now.saturating_sub(entry.used_at) > KEEP_TIME
        })
    }

    /// Frees every texture but `bound` and returns their identities: after
    /// a resize no other can be used again.
    pub(crate) fn keep_only(&mut self, bound: &[u64]) -> Vec<u64> {
        self.drain(|entry| !bound.contains(&entry.id))
    }

    fn drain(&mut self, mut gone: impl FnMut(&Entry<T>) -> bool) -> Vec<u64> {
        let mut freed = Vec::new();
        self.entries.retain(|entry| {
            let go = gone(entry);
            if go {
                freed.push(entry.id);
            }
            !go
        });
        freed
    }

    /// The specs of the textures the pool holds, in the order it made them.
    #[cfg(test)]
    pub(crate) fn held(&self) -> impl Iterator<Item = &Spec> {
        self.entries.iter().map(|entry| &entry.spec)
    }
}

/// A pooled texture's GPU objects.
pub(crate) struct PoolTexture {
    pub texture: wgpu::Texture,
    /// One view a mip level.
    views: Vec<wgpu::TextureView>,
    /// The first level through the spec's twin format, where it has one.
    twin: Option<wgpu::TextureView>,
}

impl PoolTexture {
    /// Allocates the texture `spec` describes.
    pub(crate) fn new(device: &wgpu::Device, spec: &Spec) -> Self {
        let usage = match spec.class {
            // A capture copies the frame's colour out, whichever version
            // ends it.
            Class::Color => {
                wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC
            }
            // A multisampled texture takes no other use.
            Class::Multisampled | Class::Depth => wgpu::TextureUsages::RENDER_ATTACHMENT,
            Class::Work => {
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING
            }
        };
        let label = match spec.class {
            Class::Color => "tungsten_frame_color",
            Class::Multisampled => "tungsten_scene_color_msaa",
            Class::Depth => "tungsten_scene_depth",
            Class::Work => "tungsten_frame_work",
        };
        let twin_formats: &[wgpu::TextureFormat] = match &spec.twin {
            Some(twin) => std::slice::from_ref(twin),
            None => &[],
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: spec.size.0.max(1),
                height: spec.size.1.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: spec.mips.max(1),
            sample_count: spec.samples.max(1),
            dimension: wgpu::TextureDimension::D2,
            format: spec.format,
            usage,
            view_formats: twin_formats,
        });
        let views = (0..spec.mips.max(1))
            .map(|level| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some(label),
                    base_mip_level: level,
                    mip_level_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        let twin = spec.twin.map(|format| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some(label),
                format: Some(format),
                base_mip_level: 0,
                mip_level_count: Some(1),
                ..Default::default()
            })
        });
        Self {
            texture,
            views,
            twin,
        }
    }

    /// The view of level `mip`.
    pub(crate) fn view(&self, mip: u32) -> &wgpu::TextureView {
        &self.views[mip as usize]
    }

    /// The first level as SMAA's edge pass reads it: through the non-sRGB
    /// twin where the texture has one, so it sees encoded values.
    pub(crate) fn edge_view(&self) -> &wgpu::TextureView {
        self.twin.as_ref().unwrap_or(&self.views[0])
    }
}

#[cfg(test)]
#[path = "../tests/graph_pool.rs"]
mod tests;
