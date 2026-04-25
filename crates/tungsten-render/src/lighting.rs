//! M29 lighting GPU resources.
//!
//! `LightUbo` packs at most `LIT_LIGHT_CAP` lights plus an `ambient` term
//! into a 544-byte buffer matching `std140` rules:
//!
//! | Offset | Bytes | Field |
//! | --- | --- | --- |
//! | 0     | 512   | `lights[16]` (16 × 32 bytes) |
//! | 512   | 16    | `count_pad: vec4<u32>` (`(count, 0, 0, 0)`) |
//! | 528   | 16    | `ambient: vec4<f32>` (`(rgb, 1.0)`) |
//!
//! Per-light layout (`GpuLight`, 32 bytes):
//!
//! | Offset | Bytes | Field |
//! | --- | --- | --- |
//! | 0     | 16    | `position_radius: vec4<f32>` |
//! | 16    | 16    | `color_intensity: vec4<f32>` |
//!
//! For `Point`: `position_radius = (px, py, radius, 0)`; `color_intensity.w = 0`.
//! For `Directional`: `position_radius = (cos(angle), sin(angle), 0, 0)`;
//! `color_intensity.w = 1`.

use std::cmp::Ordering;

use bytemuck::{Pod, Zeroable};
use glam::{Vec2, Vec3};
use tungsten_core::{Light, LightKind, LIGHT_CAP};

/// Mirrors `tungsten_core::LIGHT_CAP` for render-side static asserts.
pub const LIT_LIGHT_CAP: usize = LIGHT_CAP;

const _: () = assert!(LIT_LIGHT_CAP == 16);

/// Per-light POD payload (32 bytes). See module docs for field layout.
#[repr(C)]
#[derive(Debug, Pod, Zeroable, Clone, Copy, Default)]
pub struct GpuLight {
    pub position_radius: [f32; 4],
    pub color_intensity: [f32; 4],
}

/// Full per-frame light UBO (544 bytes).
#[repr(C)]
#[derive(Debug, Pod, Zeroable, Clone, Copy)]
pub struct LightUbo {
    pub lights: [GpuLight; LIT_LIGHT_CAP],
    pub count_pad: [u32; 4],
    pub ambient: [f32; 4],
}

impl Default for LightUbo {
    fn default() -> Self {
        Self {
            lights: [GpuLight::default(); LIT_LIGHT_CAP],
            count_pad: [0; 4],
            ambient: [1.0, 1.0, 1.0, 1.0],
        }
    }
}

impl LightUbo {
    #[must_use]
    pub fn byte_size() -> usize {
        std::mem::size_of::<Self>()
    }
}

/// Pack `lights` (already sorted + capped) plus `ambient` into a `LightUbo`.
#[must_use]
pub fn pack_lights(lights: &[GpuLight], ambient: Vec3) -> LightUbo {
    let mut ubo = LightUbo::default();
    let n = lights.len().min(LIT_LIGHT_CAP);
    ubo.lights[..n].copy_from_slice(&lights[..n]);
    ubo.count_pad = [n as u32, 0, 0, 0];
    ubo.ambient = [ambient.x, ambient.y, ambient.z, 1.0];
    ubo
}

/// Pack one `(position, Light)` pair into a `GpuLight`.
#[must_use]
pub fn pack_one_light(position: Vec2, light: &Light) -> GpuLight {
    match light.kind {
        LightKind::Point { radius, .. } => GpuLight {
            position_radius: [position.x, position.y, radius, 0.0],
            color_intensity: [
                light.color.x * light.intensity,
                light.color.y * light.intensity,
                light.color.z * light.intensity,
                0.0,
            ],
        },
        LightKind::Directional { angle } => GpuLight {
            position_radius: [angle.cos(), angle.sin(), 0.0, 0.0],
            color_intensity: [
                light.color.x * light.intensity,
                light.color.y * light.intensity,
                light.color.z * light.intensity,
                1.0,
            ],
        },
    }
}

/// Squared distance from `p` to the AABB rectangle `(min, max)` (0 inside).
#[must_use]
pub fn distance_to_aabb_sq(p: Vec2, min: Vec2, max: Vec2) -> f32 {
    let dx = if p.x < min.x {
        min.x - p.x
    } else if p.x > max.x {
        p.x - max.x
    } else {
        0.0
    };
    let dy = if p.y < min.y {
        min.y - p.y
    } else if p.y > max.y {
        p.y - max.y
    } else {
        0.0
    };
    dx * dx + dy * dy
}

/// Sort directional-first then nearest to the AABB rectangle, truncate to
/// `LIT_LIGHT_CAP`. Stable: ties keep input order.
#[must_use]
pub fn cull_to_cap(camera_aabb: (Vec2, Vec2), entries: &[(Vec2, Light)]) -> Vec<GpuLight> {
    let (min, max) = camera_aabb;
    let mut scored: Vec<(bool, f32, GpuLight)> = entries
        .iter()
        .map(|(pos, light)| {
            let directional = matches!(light.kind, LightKind::Directional { .. });
            let dist = distance_to_aabb_sq(*pos, min, max);
            (directional, dist, pack_one_light(*pos, light))
        })
        .collect();
    scored.sort_by(|a, b| match (a.0, b.0) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal),
    });
    scored.truncate(LIT_LIGHT_CAP);
    scored.into_iter().map(|(_, _, g)| g).collect()
}

/// Renderer-side handle on the per-frame `LightUbo` plus the bind group used
/// by the lit sprite pipeline at group 2.
pub struct LightingResources {
    pub buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    pub bind_group_layout: wgpu::BindGroupLayout,
}

impl LightingResources {
    #[must_use]
    pub fn new(device: &wgpu::Device) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lighting_ubo"),
            size: std::mem::size_of::<LightUbo>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("lighting_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lighting_bg"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });
        Self {
            buffer,
            bind_group,
            bind_group_layout,
        }
    }

    /// Upload an entire `LightUbo` payload.
    pub fn write(&self, queue: &wgpu::Queue, ubo: &LightUbo) {
        queue.write_buffer(&self.buffer, 0, bytemuck::bytes_of(ubo));
    }
}

#[cfg(test)]
#[path = "tests/lighting.rs"]
mod tests;
