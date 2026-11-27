//! M29 lighting GPU resources.
//!
//! `LightUbo` packs at most `LIT_LIGHT_CAP` lights plus an `ambient` term
//! into a 544-byte buffer matching `std140` rules:
//!
//! | Offset | Bytes | Field |
//! | --- | --- | --- |
//! | 0     | 512   | `lights[16]` (16 × 32 bytes) |
//! | 512   | 16    | `count_pad: vec4<u32>`: the lights in `lights`, the screen tiles a row, the words a tile, the lights packed in all |
//! | 528   | 16    | `ambient: vec4<f32>` (`(rgb, 1.0)`) |
//!
//! `render.light_cap` packs more than 16 (`D-310`): the lights past the
//! block ride a storage buffer beside it, in packed order, and a compute
//! pass ([`LightingResources::bin`]) gives each screen tile of
//! [`LIGHT_TILE`] pixels a bit a packed light that can reach it, which the
//! lit shader walks. A game's own lit file written against the block alone
//! still draws its 16.
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
use tungsten_core::lighting::{MAX_LIGHT_CAP, MIN_LIGHT_RADIUS};
use tungsten_core::{LIGHT_CAP, Light, LightKind};

/// Mirrors `tungsten_core::LIGHT_CAP` for render-side static asserts.
pub const LIT_LIGHT_CAP: usize = LIGHT_CAP;

const _: () = assert!(LIT_LIGHT_CAP == 16);

/// A screen tile's side in pixels (`D-310`); the lit shader's
/// `LIGHT_TILE_SHIFT` is its base-2 logarithm.
pub const LIGHT_TILE: u32 = 32;

/// The words a tile holds for a cap of `cap` lights: a bit a light.
#[must_use]
pub const fn tile_words(cap: u32) -> u32 {
    cap.div_ceil(32)
}

/// The tiles a row and a column of an output of `size` pixels.
#[must_use]
pub const fn tile_grid(size: (u32, u32)) -> (u32, u32) {
    (size.0.div_ceil(LIGHT_TILE), size.1.div_ceil(LIGHT_TILE))
}

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

/// Pack `lights` (already sorted + capped) plus `ambient` into a `LightUbo`:
/// its first 16 and their count. For a frame that walks the lights' tiles
/// the renderer adds the grid's two lanes and the count of every packed
/// light as it uploads the block.
#[must_use]
pub fn pack_lights(lights: &[GpuLight], ambient: Vec3) -> LightUbo {
    let mut ubo = LightUbo::default();
    let n = lights.len().min(LIT_LIGHT_CAP);
    ubo.lights[..n].copy_from_slice(&lights[..n]);
    ubo.count_pad = [n as u32, 0, 0, 0];
    ubo.ambient = [ambient.x, ambient.y, ambient.z, 1.0];
    ubo
}

/// A frame's packed lights (`D-310`): the block every lit shader binds, and
/// the lights past its 16, in packed order after them. Empty at the default
/// cap.
#[derive(Debug, Clone, Default)]
pub struct FrameLights {
    pub ubo: LightUbo,
    pub more: Vec<GpuLight>,
}

impl FrameLights {
    /// Packs `lights` (already sorted and capped) and `ambient`.
    #[must_use]
    pub fn pack(lights: &[GpuLight], ambient: Vec3) -> Self {
        Self {
            ubo: pack_lights(lights, ambient),
            more: lights.get(LIT_LIGHT_CAP..).unwrap_or_default().to_vec(),
        }
    }

    /// How many lights the frame packs: the block's and those past it.
    #[must_use]
    pub fn count(&self) -> u32 {
        self.ubo.count_pad[0] + self.more.len() as u32
    }
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
/// directional-first then nearest to the AABB rectangle, truncate to `cap`
/// (`render.light_cap`, taken within 1 and `MAX_LIGHT_CAP`). Stable: ties
/// keep input order.
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
pub fn cull_to_cap(
    camera_aabb: (Vec2, Vec2),
    entries: &[(Vec2, Light)],
    cap: usize,
) -> Vec<GpuLight> {
    let cap = cap.clamp(1, MAX_LIGHT_CAP);
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
    if kept.len() > cap {
        let centre = (min + max) * 0.5;
        let rank = |(pos, light): &(Vec2, Light)| {
            // A point that is not a number ranks the farthest from the
            // centre, so the order stays total and the sort can move a
            // light past it.
            let centre_sq = pos.distance_squared(centre);
            (
                u8::MAX - light.priority,
                !directional(light),
                distance_to_aabb_sq(*pos, min, max),
                if centre_sq.is_nan() {
                    f32::INFINITY
                } else {
                    centre_sq
                },
            )
        };
        let mut order: Vec<usize> = (0..kept.len()).collect();
        order.sort_by(|&a, &b| {
            rank(kept[a])
                .partial_cmp(&rank(kept[b]))
                .unwrap_or(Ordering::Equal)
        });
        order.truncate(cap);
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
    scored.truncate(cap);
    scored.into_iter().map(|(_, _, g)| g).collect()
}

/// What the binning pass reads of the frame (`light_tiles.wgsl`'s `Grid`,
/// 96 bytes): the camera's projection inverted; the output's size, a tile's
/// side and whether every light reaches every tile (`render.light_tiles`
/// off); and the tiles a row, the tiles a column, the words a tile and the
/// lights packed.
#[repr(C)]
#[derive(Debug, Pod, Zeroable, Clone, Copy, PartialEq)]
pub struct TileGrid {
    pub inverse: [[f32; 4]; 4],
    pub extent: [f32; 4],
    pub counts: [u32; 4],
}

const _: () = assert!(std::mem::size_of::<TileGrid>() == 96);

impl TileGrid {
    /// The grid of an output of `size` pixels under `view_proj`, for a cap
    /// of `cap` lights with `count` packed. Without `bins` every packed
    /// light reaches every tile.
    #[must_use]
    pub fn new(view_proj: &glam::Mat4, size: (u32, u32), cap: u32, count: u32, bins: bool) -> Self {
        let (tiles_x, tiles_y) = tile_grid(size);
        let every = if bins { 0.0 } else { 1.0 };
        Self {
            inverse: view_proj.inverse().to_cols_array_2d(),
            extent: [size.0 as f32, size.1 as f32, LIGHT_TILE as f32, every],
            counts: [tiles_x, tiles_y, tile_words(cap), count],
        }
    }

    /// Whether the binning pass tests a point light's reach: `false` where
    /// `render.light_tiles` is off.
    #[must_use]
    pub fn bins(&self) -> bool {
        self.extent[3] == 0.0
    }

    /// The world position of `pixel`, in pixels from the output's top left:
    /// the binning pass's `world_of`.
    #[must_use]
    pub fn world_of(&self, pixel: Vec2) -> Vec2 {
        let ndc = Vec2::new(
            pixel.x / self.extent[0] * 2.0 - 1.0,
            1.0 - pixel.y / self.extent[1] * 2.0,
        );
        let world =
            glam::Mat4::from_cols_array_2d(&self.inverse) * glam::Vec4::new(ndc.x, ndc.y, 0.0, 1.0);
        Vec2::new(world.x / world.w, world.y / world.w)
    }

    /// The box in world space of tile `tile`: the box of its four corners,
    /// whatever the camera's turn.
    #[must_use]
    pub fn tile_box(&self, tile: (u32, u32)) -> (Vec2, Vec2) {
        let side = self.extent[2];
        let low = Vec2::new(tile.0 as f32, tile.1 as f32) * side;
        let corners = [
            self.world_of(low),
            self.world_of(low + Vec2::new(side, 0.0)),
            self.world_of(low + Vec2::new(0.0, side)),
            self.world_of(low + Vec2::new(side, side)),
        ];
        let lo = corners.iter().copied().fold(corners[0], Vec2::min);
        let hi = corners.iter().copied().fold(corners[0], Vec2::max);
        (lo, hi)
    }
}

/// Whether the binning pass sets `light`'s bit for a tile whose box in
/// world space is `(lo, hi)`: every directional light, and a point light
/// unless its distance from the box passes its radius by more than the
/// slack. The lines of `light_tiles.wgsl` this mirrors are pinned by a
/// test.
#[must_use]
pub fn tile_reaches(lo: Vec2, hi: Vec2, light: &GpuLight) -> bool {
    if light.color_intensity[3] == 1.0 {
        return true;
    }
    let reach = lo.abs().max(hi.abs());
    let rounding = (reach.x + reach.y) * 0.0001;
    let span = (hi.x - lo.x) + (hi.y - lo.y);
    let radius = light.position_radius[2].max(0.0001);
    let at = Vec2::new(light.position_radius[0], light.position_radius[1]);
    let gap = at.distance(at.clamp(lo, hi));
    let slack = rounding + (span + radius) * 0.004;
    // As the shader writes it, so a distance that is no number keeps the
    // light.
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    let reaches = !(gap > radius + slack);
    reaches
}

/// Whether a frame walks the lights' tiles (`D-310`) under a cap of `cap`:
/// it draws a lit batch (`lit`) and packs more lights than the block's 16,
/// which only a cap past 16 lets it. Any other frame's fragments loop over
/// the block, which is the quicker while it holds every packed light.
#[must_use]
pub fn walks_tiles(cap: u32, lit: bool, packed: u32) -> bool {
    lit && packed.min(cap) > LIT_LIGHT_CAP as u32
}

/// The binning pass's source: internal, as `quad` and `present_blit` are.
pub(crate) const LIGHT_TILES_SOURCE: &str = include_str!("light_tiles.wgsl");

/// What the light buffers hold, so that a frame uploads only what they do
/// not (`D-310`): the block, the lights past it from their buffer's start,
/// and the binning pass's grid.
#[derive(Debug, Default)]
pub(crate) struct Held {
    ubo: Option<LightUbo>,
    more: Vec<GpuLight>,
    grid: Option<TileGrid>,
}

/// The buffers a frame uploads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Uploads {
    pub ubo: bool,
    pub more: bool,
    pub grid: bool,
}

impl Held {
    /// What a frame uploads whose block is `ubo` and which, where it walks
    /// tiles, reads `walk`'s lights past the block and grid; the buffers
    /// hold them after it. A frame that walks none leaves those two as they
    /// are: nothing reads them. The lights past the block are held already
    /// where they are the first of what their buffer was last written.
    pub(crate) fn take(
        &mut self,
        ubo: &LightUbo,
        walk: Option<(&[GpuLight], &TileGrid)>,
    ) -> Uploads {
        let mut uploads = Uploads::default();
        let same = |held: &LightUbo| bytemuck::bytes_of(held) == bytemuck::bytes_of(ubo);
        if !self.ubo.as_ref().is_some_and(same) {
            self.ubo = Some(*ubo);
            uploads.ubo = true;
        }
        if let Some((more, grid)) = walk {
            let bytes: &[u8] = bytemuck::cast_slice(more);
            if !bytemuck::cast_slice::<GpuLight, u8>(&self.more).starts_with(bytes) {
                self.more.clear();
                self.more.extend_from_slice(more);
                uploads.more = true;
            }
            if self.grid != Some(*grid) {
                self.grid = Some(*grid);
                uploads.grid = true;
            }
        }
        uploads
    }
}

/// What a cap past the block's 16 adds (`D-310`): the binning pass and its
/// grid. A renderer at the default cap has none.
struct Binning {
    layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::ComputePipeline,
    /// The pass's [`TileGrid`].
    grid: wgpu::Buffer,
    /// `render.light_tiles`: whether the pass tests a light's reach.
    bins: bool,
    /// The tiles a row and a column the tiles buffer is sized for.
    tiles_in: (u32, u32),
}

/// Renderer-side handle on a frame's lights (`D-061`, `D-310`): the
/// `LightUbo`, the lights past its 16 and the screen tiles' words, which the
/// lit pipeline binds at group 2, and under a cap past 16 the compute pass
/// that writes the words.
pub struct LightingResources {
    pub buffer: wgpu::Buffer,
    /// The packed lights past the block's 16: `cap - 16` of them, one at
    /// least.
    more: wgpu::Buffer,
    /// Each screen tile's words, a row after a row, sized for the output
    /// under a cap past 16; one word otherwise, which nothing reads.
    tiles: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    pub bind_group_layout: wgpu::BindGroupLayout,
    binning: Option<Binning>,
    /// `render.light_cap`.
    cap: u32,
    /// What the buffers hold, so an unchanged frame uploads nothing.
    held: Held,
    /// Whether a frame has walked tiles yet: the first one says so.
    walked: bool,
    pub(crate) counts: Box<Counts>,
}

/// The bytes of a tiles buffer for a grid of `tiles` under a cap of `cap`.
fn tiles_bytes(tiles: (u32, u32), cap: u32) -> u64 {
    u64::from(tiles.0.max(1)) * u64::from(tiles.1.max(1)) * u64::from(tile_words(cap)) * 4
}

fn buffer_entry(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
        resource: buffer.as_entire_binding(),
    }
}

impl LightingResources {
    /// The resources for a cap of `cap` lights (`render.light_cap`, taken
    /// within 1 and `MAX_LIGHT_CAP`) and an output of `size` pixels. `bins`
    /// is `render.light_tiles`. At a cap of 16 or under there is no binning
    /// pass: every frame draws with the block alone.
    #[must_use]
    pub fn new(device: &wgpu::Device, cap: u32, size: (u32, u32), bins: bool) -> Self {
        let cap = cap.clamp(1, MAX_LIGHT_CAP as u32);
        let tiled = cap > LIT_LIGHT_CAP as u32;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lighting_ubo"),
            size: std::mem::size_of::<LightUbo>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let past = cap.saturating_sub(LIT_LIGHT_CAP as u32).max(1);
        let more = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lighting_more"),
            size: u64::from(past) * std::mem::size_of::<GpuLight>() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform = |binding: u32, visibility: wgpu::ShaderStages| wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let storage = |binding: u32, visibility: wgpu::ShaderStages, read_only: bool| {
            wgpu::BindGroupLayoutEntry {
                binding,
                visibility,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }
        };
        // Group 2 of the lit pipeline. Binding 0 is as it was, so a game's
        // own lit file written against it alone still builds.
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("lighting_bgl"),
            entries: &[
                uniform(0, wgpu::ShaderStages::VERTEX_FRAGMENT),
                storage(1, wgpu::ShaderStages::FRAGMENT, true),
                storage(2, wgpu::ShaderStages::FRAGMENT, true),
            ],
        });
        let tiles_in = if tiled { tile_grid(size) } else { (1, 1) };
        let tiles = Self::tiles_buffer(device, tiles_in, if tiled { cap } else { 1 });
        let bind_group = Self::lit_group(device, &bind_group_layout, &buffer, &more, &tiles);
        let binning = tiled.then(|| {
            let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("light_tiles_bgl"),
                entries: &[
                    uniform(0, wgpu::ShaderStages::COMPUTE),
                    storage(1, wgpu::ShaderStages::COMPUTE, true),
                    uniform(2, wgpu::ShaderStages::COMPUTE),
                    storage(3, wgpu::ShaderStages::COMPUTE, false),
                ],
            });
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("light_tiles"),
                source: wgpu::ShaderSource::Wgsl(LIGHT_TILES_SOURCE.into()),
            });
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("light_tiles_layout"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
            let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("light_tiles_pipeline"),
                layout: Some(&pipeline_layout),
                module: &module,
                entry_point: Some("main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            });
            let grid = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("lighting_grid"),
                size: std::mem::size_of::<TileGrid>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind_group = Self::bin_group(device, &layout, &buffer, &more, &grid, &tiles);
            Binning {
                layout,
                bind_group,
                pipeline,
                grid,
                bins,
                tiles_in,
            }
        });
        Self {
            buffer,
            more,
            tiles,
            bind_group,
            bind_group_layout,
            binning,
            cap,
            held: Held::default(),
            walked: false,
            counts: Counts::boxed(),
        }
    }

    fn tiles_buffer(device: &wgpu::Device, tiles: (u32, u32), cap: u32) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lighting_tiles"),
            size: tiles_bytes(tiles, cap),
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        })
    }

    /// The lit pipeline's group over the block, the lights past it and the
    /// tiles' words.
    fn lit_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        ubo: &wgpu::Buffer,
        more: &wgpu::Buffer,
        tiles: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lighting_bg"),
            layout,
            entries: &[
                buffer_entry(0, ubo),
                buffer_entry(1, more),
                buffer_entry(2, tiles),
            ],
        })
    }

    /// The binning pass's group over the same buffers and its grid.
    fn bin_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        ubo: &wgpu::Buffer,
        more: &wgpu::Buffer,
        grid: &wgpu::Buffer,
        tiles: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("light_tiles_bg"),
            layout,
            entries: &[
                buffer_entry(0, ubo),
                buffer_entry(1, more),
                buffer_entry(2, grid),
                buffer_entry(3, tiles),
            ],
        })
    }

    /// `render.light_cap`, as the resources took it.
    #[must_use]
    pub fn cap(&self) -> u32 {
        self.cap
    }

    /// Whether a frame that draws a lit batch (`lit`) and packs `packed`
    /// lights walks tiles ([`walks_tiles`] at this cap).
    #[must_use]
    pub fn walks_tiles(&self, lit: bool, packed: u32) -> bool {
        self.binning.is_some() && walks_tiles(self.cap, lit, packed)
    }

    /// Uploads a frame's lights, each buffer only where it does not hold
    /// them already. A frame that walks `tiles` ([`Self::walks_tiles`]) also
    /// takes the lights past the block and the grid of its screen tiles for
    /// an output of `size` pixels under `view_proj`, the tiles buffer
    /// following the output's size; the block then carries the grid's two
    /// lanes and the count of every packed light. Any other frame uploads
    /// the block as it always was.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        lights: &FrameLights,
        view_proj: &glam::Mat4,
        size: (u32, u32),
        tiles: bool,
    ) {
        let past = self.cap.saturating_sub(LIT_LIGHT_CAP as u32) as usize;
        let more = &lights.more[..lights.more.len().min(past)];
        let mut ubo = lights.ubo;
        ubo.count_pad = [ubo.count_pad[0], 0, 0, 0];
        let Some(binning) = self.binning.as_mut().filter(|_| tiles) else {
            if self.held.take(&ubo, None).ubo {
                counters::write_buffer(
                    &self.counts,
                    queue,
                    &self.buffer,
                    0,
                    bytemuck::bytes_of(&ubo),
                );
            }
            return;
        };
        let tiles_in = tile_grid(size);
        ubo.count_pad[1] = tiles_in.0;
        ubo.count_pad[2] = tile_words(self.cap);
        ubo.count_pad[3] = ubo.count_pad[0] + more.len() as u32;
        if !self.walked {
            self.walked = true;
            log::info!(
                "lights: a frame packs {} lights, more than the block's {LIT_LIGHT_CAP}: its lit batches walk the lights' tiles",
                ubo.count_pad[3]
            );
        }
        if tiles_in != binning.tiles_in {
            self.tiles = Self::tiles_buffer(device, tiles_in, self.cap);
            binning.tiles_in = tiles_in;
            self.bind_group = Self::lit_group(
                device,
                &self.bind_group_layout,
                &self.buffer,
                &self.more,
                &self.tiles,
            );
            binning.bind_group = Self::bin_group(
                device,
                &binning.layout,
                &self.buffer,
                &self.more,
                &binning.grid,
                &self.tiles,
            );
        }
        let grid = TileGrid::new(view_proj, size, self.cap, ubo.count_pad[3], binning.bins);
        let uploads = self.held.take(&ubo, Some((more, &grid)));
        if uploads.ubo {
            counters::write_buffer(
                &self.counts,
                queue,
                &self.buffer,
                0,
                bytemuck::bytes_of(&ubo),
            );
        }
        if uploads.more {
            counters::write_buffer(
                &self.counts,
                queue,
                &self.more,
                0,
                bytemuck::cast_slice(more),
            );
        }
        if uploads.grid {
            counters::write_buffer(
                &self.counts,
                queue,
                &binning.grid,
                0,
                bytemuck::bytes_of(&grid),
            );
        }
    }

    /// Records the binning pass's dispatch into `pass`: one invocation a
    /// tile, in groups of 8 by 8. Nothing without a cap past 16.
    pub(crate) fn bin(&self, pass: &mut wgpu::ComputePass<'_>) {
        let Some(binning) = &self.binning else {
            return;
        };
        pass.set_pipeline(&binning.pipeline);
        pass.set_bind_group(0, &binning.bind_group, &[]);
        pass.dispatch_workgroups(
            binning.tiles_in.0.max(1).div_ceil(8),
            binning.tiles_in.1.max(1).div_ceil(8),
            1,
        );
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
