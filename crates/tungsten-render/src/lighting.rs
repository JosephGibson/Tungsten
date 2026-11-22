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

use crate::counters::{self, Counts};
use bytemuck::{Pod, Zeroable};
use glam::{Vec2, Vec3};
use tungsten_core::lighting::MIN_LIGHT_RADIUS;
use tungsten_core::{LIGHT_CAP, Light, LightKind};

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

const _: () = assert!(std::mem::size_of::<GpuLight>() == 32);
const _: () = assert!(std::mem::size_of::<LightUbo>() == 544);

impl Default for LightUbo {
    fn default() -> Self {
        Self {
            lights: [GpuLight::default(); LIT_LIGHT_CAP],
            count_pad: [0; 4],
            ambient: [1.0, 1.0, 1.0, 1.0],
        }
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
        LightKind::Point { radius } => GpuLight {
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

/// How far past its radius a point light is still taken to reach the view:
/// the test runs on the CPU and the shader's attenuation on the GPU, so a
/// light is dropped only when it clears the view by more than their rounding.
const REACH_MARGIN: f32 = 1.0001;

/// Whether `light` at `position` can change a pixel of the view
/// `camera_aabb`, as the lit shader draws it: a dark light (intensity 0)
/// adds nothing anywhere, and a point light adds nothing to a fragment its
/// radius, taken as at least `MIN_LIGHT_RADIUS`, does not reach, so one
/// whose radius ends short of the view lights no fragment in it. A
/// directional light that is not dark always can.
///
/// What is not a number is kept: a light whose colour times its intensity
/// is not finite, since the shader's product with it is no zero, and a
/// point light whose radius is a NaN. A NaN coordinate, the light's or the
/// view's, counts as inside the view on its axis.
#[must_use]
pub fn lights_view(camera_aabb: (Vec2, Vec2), position: Vec2, light: &Light) -> bool {
    if !(light.color * light.intensity).is_finite() {
        return true;
    }
    if light.intensity == 0.0 {
        return false;
    }
    match light.kind {
        LightKind::Directional { .. } => true,
        LightKind::Point { radius } => {
            let (min, max) = camera_aabb;
            // Not `f32::max`, which would take a NaN radius for the minimum.
            let radius = if radius < MIN_LIGHT_RADIUS {
                MIN_LIGHT_RADIUS
            } else {
                radius
            };
            let reach = radius * REACH_MARGIN;
            // Dropped only when it is farther than its reach: a NaN is
            // neither farther nor nearer, so a NaN radius is kept.
            distance_to_aabb_sq(position, min, max).partial_cmp(&(reach * reach))
                != Some(Ordering::Greater)
        }
    }
}

/// Keep the lights that can light the view ([`lights_view`]), sort
/// directional-first then nearest to the AABB rectangle, truncate to
/// `LIT_LIGHT_CAP`. Stable: ties keep input order.
///
/// Past the cap, the lights kept are chosen first: by [`Light::priority`],
/// the higher kept first, so the light a game must never lose, a player's
/// lantern, keeps its slot while it lights the view; then directional before
/// point, then nearer the view, and among lights the same distance from it,
/// all those inside it, the one nearer the view's centre, so a crowd at the
/// view's edges cannot push out the light the player stands in. The kept
/// lights then sort as above: a priority chooses which lights are packed,
/// never their order, and changes nothing while the cap holds them all.
#[must_use]
pub fn cull_to_cap(camera_aabb: (Vec2, Vec2), entries: &[(Vec2, Light)]) -> Vec<GpuLight> {
    let (min, max) = camera_aabb;
    let directional = |light: &Light| matches!(light.kind, LightKind::Directional { .. });
    // Sized for every entry, as a list of them all was: a filtered
    // collect would grow it by doubling.
    let mut kept: Vec<&(Vec2, Light)> = Vec::with_capacity(entries.len());
    kept.extend(
        entries
            .iter()
            .filter(|(pos, light)| lights_view(camera_aabb, *pos, light)),
    );
    if kept.len() > LIT_LIGHT_CAP {
        let centre = (min + max) * 0.5;
        let rank = |(pos, light): &(Vec2, Light)| {
            (
                u8::MAX - light.priority,
                !directional(light),
                distance_to_aabb_sq(*pos, min, max),
                pos.distance_squared(centre),
            )
        };
        let mut order: Vec<usize> = (0..kept.len()).collect();
        order.sort_by(|&a, &b| {
            rank(kept[a])
                .partial_cmp(&rank(kept[b]))
                .unwrap_or(Ordering::Equal)
        });
        order.truncate(LIT_LIGHT_CAP);
        order.sort_unstable();
        kept = order.into_iter().map(|index| kept[index]).collect();
    }
    let mut scored: Vec<(bool, f32, GpuLight)> = kept
        .into_iter()
        .map(|(pos, light)| {
            let dist = distance_to_aabb_sq(*pos, min, max);
            (directional(light), dist, pack_one_light(*pos, light))
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
    pub(crate) counts: Box<Counts>,
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
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
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
            counts: Counts::boxed(),
        }
    }

    /// Upload an entire `LightUbo` payload.
    pub fn write(&self, queue: &wgpu::Queue, ubo: &LightUbo) {
        counters::write_buffer(
            &self.counts,
            queue,
            &self.buffer,
            0,
            bytemuck::bytes_of(ubo),
        );
    }
}

#[cfg(test)]
#[path = "tests/lighting.rs"]
mod tests;
