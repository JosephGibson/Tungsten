//! Default sprite extract: `Transform + Sprite + Visibility` -> [`SpriteBatch`].
//! The default sprite channel of [`Extracts`](crate::Extracts) draws the
//! tilemaps, then this.
//!
//! D-042: explicit `Visibility` required. Order: `(z_order, entity.id)`,
//! batched within z-runs. `z_norm` is derived from the same painter ordering
//! so `DepthSortMode::GpuDepth` reproduces the CPU-visible order.
//!
//! M26: batch key extends with `(material_id, uniform_overrides_hash)` so
//! per-entity material animations never alias through one UBO upload. Same-
//! material same-override batches still collapse; the M25 default bytes are
//! byte-identical when no sprite carries `material_id`.
//!
//! M30 (`D-073`): a `ParallaxLayer` entity's instance position is remapped
//! against `CameraState.position` at extract time, so one view-projection still
//! draws every layer. The remap touches positions only — `BatchKey` and `z_norm`
//! are unchanged, and a sprite without `ParallaxLayer` extracts byte-identically
//! to pre-M30.
//!
//! Per-sprite cost: one pass over the query writes an instance and a 16-byte
//! sort key per visible sprite; the keys are sorted (skipped when the query
//! already yields painter order) and a second pass gathers the instances into
//! their batches. A frame that is one batch in query order skips the second
//! pass: its instance list becomes the batch. The frame after such a one, on
//! kept buffers, starts keyless (7.14's C6, `D-294`): it writes a key for a
//! culled sprite alone, and when a sprite breaks the one batch (a second
//! class, a second `z_order`, a lower order) pass 1 runs again with keys,
//! once, and the next sixty frames start keyed. A sprite's interned asset ID
//! indexes the registry directly, with no hashing or string compare. An
//! archetype with no override block and no flip, the common one, takes a row
//! loop of its own (7.14's C6), which keeps what a sprite ID and material
//! resolve to while the next row names the same pair; the rest take the
//! general loop, and both write the same keys and instances.
//!
//! A sprite ID with no loaded asset, or a lit sprite carrying a material, logs
//! a warning once per ID for as long as `ExtractScratch` lives (every
//! `App`); the flags and the log sit in a cold helper outside the row loop.
//!
//! Redo 7.9 also draws a tile layer with a `z` among the sprites, for the
//! default base: its batches before the first z-run at or above its `z`,
//! each layer one place in the painter order, so under `gpu_depth` it takes
//! that place's depth. Without such a layer nothing changes.
//!
//! Redo 7.9: a `SpriteFlip` holding `true` mirrors the instance by a negative
//! UV width, about the sprite's anchor or, without one, in its own box; an
//! `Unlit` sprite with maps draws on the stock pipeline; a lit sprite keeps
//! its `UniformOverrideBlock`, whose flash lanes the lit shader reads. An
//! archetype holds a flip or the marker in every row or in none, so its first
//! row tells, and a frame without either looks nothing more up.
//!
//! `D-114`: a sprite drawn by the stock sprite or lit pipeline whose bound lies
//! wholly outside the camera's view writes no instance. The view is the one
//! render projects with: the surface size the app passes through
//! `ExtractScratch`, else `WindowSize`; without a camera or a size nothing is
//! culled. A culled sprite keeps its class and sort key, so batches open where
//! they did, `z_norm` counts it, and every kept instance and the order of every
//! non-empty batch equal the uncull extract's; batches culling empties are
//! dropped. Material sprites run their shader's own vertex stage and are kept.
//! With `ExtractScratch` in the world every buffer, the batches' instance
//! vectors included, is reused across frames. Output bytes are unchanged from
//! the tuple-sorting extract this replaced; `tests/sprite_extract.rs` keeps
//! that extract as the reference.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::hash::Hasher;

use glam::{BVec2, Vec2};
use tungsten_core::assets::{SpriteAssetId, TextureHandle};
use tungsten_core::components::{Backdrop, BackdropRepeat, SpriteFlip, Unlit};
use tungsten_core::tween::UniformOverrideBlock;
use tungsten_core::{
    AssetRegistry, CameraState, Entity, FilterMode, MaterialAssetId, OptionalColumn, ParallaxLayer,
    Sprite, SpriteAsset, Time, Transform, Visibility, World, parallax_world_position,
};
use tungsten_render::{SpriteBatch, SpriteInstance};

use crate::app::WindowSize;
use crate::tilemap_extract::RaisedLayer;

/// Hash of a `UniformOverrideBlock`'s 256-byte payload, used as a batch-split
/// key so per-entity overrides cannot alias through one UBO upload.
fn override_key(block: &UniformOverrideBlock) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    hasher.write(&block.to_bytes());
    hasher.finish()
}

type BatchKey = (u32, FilterMode, Option<MaterialAssetId>, Option<u64>, bool);

/// One visible sprite in the sort: its painter order, its slot in the record
/// list and its batch class.
#[derive(Clone, Copy)]
struct SortKey {
    /// `(z_order, entity id)` packed so integer order is painter order.
    order: u64,
    /// Slot in the record list, or [`CULLED`]. A keyless frame writes keys
    /// for its culled sprites alone, each holding the record count at its
    /// place: the kept sprites before it.
    record: u32,
    class: u32,
}

/// `SortKey::record` of a culled sprite, which has no record.
const CULLED: u32 = u32::MAX;

/// Flipping the sign bit maps `i32` order onto `u32` order. Entity IDs are
/// unique among live entities, so no two sprites share a value.
fn painter_order(z_order: i32, entity_id: u32) -> u64 {
    (u64::from(z_order as u32 ^ 0x8000_0000) << 32) | u64::from(entity_id)
}

/// Map painter order so the depth test reproduces "later-drawn wins". With
/// `depth_compare = LessEqual` and a 1.0 clear, a fragment passes when its
/// z ≤ the current depth. Keeping the first-drawn (most distant) at a larger z
/// and the last-drawn (closest) at 0 lets every subsequent overlap pass the
/// test, so the final visible pixel matches the CpuStable painter output.
#[inline]
fn z_norm(total: usize, idx_in_order: usize) -> f32 {
    (total as f32 - 1.0 - idx_in_order as f32) / total as f32
}

/// One distinct [`BatchKey`] of the frame: what a batch opened for it carries,
/// and the batch it opened in the current z-run.
struct BatchClass {
    texture: TextureHandle,
    filter: FilterMode,
    material_id: Option<MaterialAssetId>,
    uniform_overrides: Option<UniformOverrideBlock>,
    lit: bool,
    /// Z-run of `batch`; runs count from 1.
    run: u32,
    batch: u32,
}

/// Capacity classes of [`InstancePool`]: class `c` holds vectors with room
/// for at least `1 << c` instances.
const POOL_CLASSES: usize = 32;

/// Emptied instance vectors of the last frame's batches, binned by capacity,
/// so a batch gets a vector that fits it whatever order batches are built or
/// drawn in. Vectors a frame doesn't take are freed at the next refill.
#[derive(Default)]
pub(crate) struct InstancePool {
    bins: [Vec<Vec<SpriteInstance>>; POOL_CLASSES],
}

impl InstancePool {
    /// An empty vector with room for `len` instances: a pooled one of that
    /// capacity class or the next, else a new one.
    pub(crate) fn take(&mut self, len: usize) -> Vec<SpriteInstance> {
        let class = len.max(4).next_power_of_two().trailing_zeros() as usize;
        if class >= POOL_CLASSES {
            return Vec::with_capacity(len);
        }
        for bin in self.bins[class..].iter_mut().take(2) {
            if let Some(instances) = bin.pop() {
                return instances;
            }
        }
        Vec::with_capacity(1 << class)
    }

    fn refill(&mut self, batches: &mut Vec<SpriteBatch>) {
        for bin in &mut self.bins {
            bin.clear();
        }
        for batch in batches.drain(..) {
            let mut instances = batch.instances;
            let capacity = instances.capacity();
            if capacity == 0 {
                continue;
            }
            instances.clear();
            let class = (capacity.ilog2() as usize).min(POOL_CLASSES - 1);
            self.bins[class].push(instances);
        }
    }
}

/// Sprite IDs a warning has named, one bit per [`SpriteAssetId`] index.
#[derive(Default)]
pub(crate) struct WarnedIds(Vec<u64>);

impl WarnedIds {
    /// Marks `id`; true the first time.
    fn first(&mut self, id: SpriteAssetId) -> bool {
        let index = id.index() as usize;
        let (word, bit) = (index / 64, 1u64 << (index % 64));
        if word >= self.0.len() {
            self.0.resize(word + 1, 0);
        }
        let first = self.0[word] & bit == 0;
        self.0[word] |= bit;
        first
    }

    #[cfg(test)]
    fn contains(&self, id: SpriteAssetId) -> bool {
        let index = id.index() as usize;
        self.0
            .get(index / 64)
            .is_some_and(|word| word & (1u64 << (index % 64)) != 0)
    }
}

/// What the default extract warns about, once per sprite ID.
#[derive(Clone, Copy)]
enum SpriteWarning {
    /// The ID names no loaded sprite, so the sprite draws nothing.
    NoAsset,
    /// A lit sprite carries this material; the lit shader draws it, taking
    /// only its block's flash lanes (`D-061`).
    LitMaterial(Option<MaterialAssetId>),
    /// A backdrop of this sprite repeats past [`BACKDROP_COPIES`] over the
    /// view, which drops the rest.
    BackdropCopies,
}

/// Logs `warning` about sprite `id` the first time these buffers meet it.
/// Cold and out of line: the row loop keeps only the branch tests that lead
/// here.
#[cold]
#[inline(never)]
fn warn_once(
    buffers: &mut SpriteWarnings,
    assets: &AssetRegistry,
    id: SpriteAssetId,
    warning: SpriteWarning,
) {
    let name = assets.sprite_name(id).unwrap_or("?");
    match warning {
        SpriteWarning::NoAsset => {
            if buffers.no_asset.first(id) {
                log::warn!("sprite '{name}' has no loaded asset; it draws nothing");
            }
        }
        SpriteWarning::LitMaterial(material) => {
            if buffers.lit_material.first(id) {
                log::warn!(
                    "lit sprite '{name}' carries material_id {material:?}; the lit shader draws it, taking only its block's flash lanes"
                );
            }
        }
        SpriteWarning::BackdropCopies => {
            if buffers.backdrop_copies.first(id) {
                log::warn!(
                    "backdrop sprite '{name}' repeats past {BACKDROP_COPIES} copies over the view; the rest draw nothing"
                );
            }
        }
    }
}

/// The sprite IDs each warning of the default extract has named.
#[derive(Default)]
pub(crate) struct SpriteWarnings {
    no_asset: WarnedIds,
    lit_material: WarnedIds,
    backdrop_copies: WarnedIds,
}

/// Buffers of the default sprite extract and the tilemap extract.
#[derive(Default)]
pub(crate) struct ExtractBuffers {
    keys: Vec<SortKey>,
    /// Second buffer of the radix sort.
    keys_swap: Vec<SortKey>,
    /// One instance per key, `z_norm` unset, in query order.
    records: Vec<SpriteInstance>,
    classes: Vec<BatchClass>,
    class_of: HashMap<BatchKey, u32>,
    /// Instance counts of the batches the sprite extract built last time, in
    /// order: the capacity each batch asks the pool for.
    sprite_batch_lens: Vec<u32>,
    /// The same for the tilemap extract.
    pub(crate) tile_batch_lens: Vec<u32>,
    /// Per-map tileset cache of the tilemap extract.
    pub(crate) tileset: crate::tilemap_extract::TilesetCache,
    pub(crate) pool: InstancePool,
    /// The last frame's emptied batch list.
    batch_list: Vec<SpriteBatch>,
    /// Sprite IDs already warned about, so each warning comes once.
    warnings: SpriteWarnings,
    /// Tilemap IDs the tilemap extract found no map for, warned about once
    /// each.
    pub(crate) unknown_tilemaps: Vec<String>,
    /// The sprites the last frame's pass 1 counted, culled ones included:
    /// the room its instance vector asks the pool for and the count
    /// `z_norm`'s first guess assumes. A keyless frame writes no key a kept
    /// sprite, so the keys' length no longer tells.
    last_total: usize,
    /// Whether the last frame ended as one batch, so this one may start
    /// keyless.
    one_batch: bool,
    /// Frames left that start keyed whatever the last one was:
    /// [`KEYED_HOLD`] after a restart.
    keyed_hold: u8,
    /// Runs pass 1's general loop over every archetype, as the extract did
    /// before plain archetypes took a loop of their own: the tests' reference
    /// for that loop.
    #[cfg(test)]
    general_only: bool,
    /// Starts every frame keyed, as the extract did before the keyless
    /// frame: the tests' reference for it.
    #[cfg(test)]
    keyed_only: bool,
    /// Frames that ran pass 1 again with keys, for the tests.
    #[cfg(test)]
    restarts: u32,
    /// Frames that ended keyless, for the tests.
    #[cfg(test)]
    keyless_frames: u32,
}

/// Frames that start keyed after a keyless pass 1 ran again with keys, so a
/// game that flips between one batch and several restarts once in sixty.
const KEYED_HOLD: u8 = 60;

/// World resource that lets [`extract_sprites_default`] and the tilemap
/// extracts ([`extract_tilemaps`](crate::extract_tilemaps) and
/// [`extract_tilemap_layers`](crate::extract_tilemap_layers)) reuse their
/// buffers across frames. `App` inserts it and hands each frame's batches
/// back after the render stage; without it the extracts allocate per call.
/// It also carries the surface size render projects with, which the sprite
/// extract culls against (`D-114`).
#[derive(Default)]
pub(crate) struct ExtractScratch(RefCell<ExtractBuffers>, Cell<Option<(u32, u32)>>);

impl ExtractScratch {
    /// Runs `f` on the scratch buffers of `world`, or on buffers of its own
    /// when the resource is absent or already borrowed (a nested extract).
    pub(crate) fn with<R>(world: &World, f: impl FnOnce(&mut ExtractBuffers) -> R) -> R {
        match world
            .get_resource::<Self>()
            .and_then(|scratch| scratch.0.try_borrow_mut().ok())
        {
            Some(mut buffers) => f(&mut buffers),
            None => f(&mut ExtractBuffers::default()),
        }
    }

    /// Records the surface size render projects this frame with. A pending
    /// resize leaves `WindowSize` ahead of it.
    pub(crate) fn set_viewport(&self, width: u32, height: u32) {
        self.1.set(Some((width, height)));
    }

    fn viewport(&self) -> Option<(u32, u32)> {
        self.1.get()
    }

    /// Takes a drawn frame's batches back: their instance vectors serve the
    /// next frame's batches.
    pub(crate) fn recycle(&self, mut batches: Vec<SpriteBatch>) {
        let mut buffers = self.0.borrow_mut();
        buffers.pool.refill(&mut batches);
        buffers.batch_list = batches;
    }
}

/// Default sprite extract: the `Sprite` entities, which the default sprite
/// channel of [`Extracts`](crate::Extracts) draws after the tilemaps.
#[must_use]
pub fn extract_sprites_default(world: &World) -> Vec<SpriteBatch> {
    let Some(assets) = world.get_resource::<AssetRegistry>() else {
        return Vec::new();
    };
    let view = view_bounds(world);
    ExtractScratch::with(world, |buffers| {
        extract_into(world, assets, view, buffers, &mut [])
    })
}

/// [`extract_sprites_default`] with `raised`, tile layers sorted by `z`,
/// drawn among the sprites: each before the first sprite whose `z_order` is
/// at or above its `z`, the rest after every sprite.
pub(crate) fn extract_sprites_among(world: &World, raised: &mut [RaisedLayer]) -> Vec<SpriteBatch> {
    let Some(assets) = world.get_resource::<AssetRegistry>() else {
        return Vec::new();
    };
    let view = view_bounds(world);
    ExtractScratch::with(world, |buffers| {
        extract_into(world, assets, view, buffers, raised)
    })
}

/// Most copies one backdrop draws in a frame.
const BACKDROP_COPIES: usize = 1 << 16;

/// The default base's backdrops ([`Backdrop`]), by `z_order` then entity:
/// their copies over the camera's view, on the stock pipeline at the far
/// plane. Empty, and no buffer touched, without a backdrop.
pub(crate) fn extract_backdrops(world: &World) -> Vec<SpriteBatch> {
    let mut found: Vec<(i32, u32, Backdrop, Transform)> = world
        .query::<(Entity, &Backdrop, &Transform, Option<&Visibility>)>()
        .filter(|(.., visibility)| visibility.is_none_or(|visibility| visibility.visible))
        .map(|(entity, backdrop, transform, _)| {
            (backdrop.z_order, entity.id(), *backdrop, *transform)
        })
        .collect();
    let Some(assets) = world.get_resource::<AssetRegistry>() else {
        return Vec::new();
    };
    if found.is_empty() {
        return Vec::new();
    }
    found.sort_by_key(|&(z_order, entity, ..)| (z_order, entity));
    let camera = world
        .get_resource::<CameraState>()
        .map_or(Vec2::ZERO, |camera| camera.position);
    let view = view_bounds(world);
    let elapsed = world.get_resource::<Time>().map_or(0.0, Time::elapsed);
    ExtractScratch::with(world, |buffers| {
        let mut out: Vec<SpriteBatch> = Vec::new();
        for (_, _, backdrop, transform) in found {
            let Some(asset) = assets.sprite(backdrop.sprite) else {
                warn_once(
                    &mut buffers.warnings,
                    assets,
                    backdrop.sprite,
                    SpriteWarning::NoAsset,
                );
                continue;
            };
            let size = Vec2::new(asset.width as f32, asset.height as f32) * transform.scale.abs();
            if size.x <= 0.0 || size.y <= 0.0 {
                continue;
            }
            let uv_min = Vec2::from(asset.uv.min);
            let uv_size = Vec2::from(asset.uv.max) - uv_min;
            let batch = match out.last_mut() {
                Some(batch) if batch.texture == asset.atlas && batch.filter == asset.filter => {
                    batch
                }
                _ => {
                    out.push(SpriteBatch::new(asset.atlas, asset.filter));
                    out.last_mut().unwrap()
                }
            };
            let copy = |position: Vec2, size: Vec2, uv_min: Vec2, uv_size: Vec2| SpriteInstance {
                position: position.to_array(),
                size: size.to_array(),
                rotation: 0.0,
                color: backdrop.tint,
                uv_min: uv_min.to_array(),
                uv_size: uv_size.to_array(),
                z_norm: 1.0,
                _pad: 0.0,
            };
            if backdrop.fill
                && let Some((min, max)) = view
            {
                batch.instances.push(copy(min, max - min, uv_min, uv_size));
                continue;
            }
            let (across, down) = match backdrop.repeat {
                BackdropRepeat::None => (false, false),
                BackdropRepeat::X => (true, false),
                BackdropRepeat::Y => (false, true),
                BackdropRepeat::Xy => (true, true),
            };
            // The drift on the game clock, within one copy along a repeat so
            // a long run keeps its precision.
            let drift = |speed: f32, period: f32, repeats: bool| {
                let travelled = f64::from(speed) * elapsed;
                let travelled = if repeats {
                    travelled.rem_euclid(f64::from(period))
                } else {
                    travelled
                };
                travelled as f32
            };
            let origin = parallax_world_position(transform.position, backdrop.factor, camera)
                + Vec2::new(
                    drift(backdrop.drift.x, size.x, across),
                    drift(backdrop.drift.y, size.y, down),
                );
            let span = |start: f32, step: f32, low: f32, high: f32, repeats: bool| {
                if repeats {
                    let first = ((low - start) / step).floor() as i64;
                    first..((high - start) / step).ceil().max(first as f32 + 1.0) as i64
                } else {
                    0..1
                }
            };
            let (columns, rows) = match view {
                Some((min, max)) => (
                    span(origin.x, size.x, min.x, max.x, across),
                    span(origin.y, size.y, min.y, max.y, down),
                ),
                None => (0..1, 0..1),
            };
            let copies = columns.clone().count().saturating_mul(rows.clone().count());
            if copies > BACKDROP_COPIES {
                warn_once(
                    &mut buffers.warnings,
                    assets,
                    backdrop.sprite,
                    SpriteWarning::BackdropCopies,
                );
            }
            let mut left = BACKDROP_COPIES;
            'copies: for row in rows {
                for column in columns.clone() {
                    if left == 0 {
                        break 'copies;
                    }
                    left -= 1;
                    let at = origin + Vec2::new(column as f32, row as f32) * size;
                    batch.instances.push(copy(at, size, uv_min, uv_size));
                    // The foot: the last texel row's centre, stretched from
                    // the copy's bottom to the view's.
                    if backdrop.foot
                        && !down
                        && let Some((_, max)) = view
                        && max.y > at.y + size.y
                    {
                        let row_centre = uv_min.y + uv_size.y * (1.0 - 0.5 / asset.height as f32);
                        batch.instances.push(copy(
                            Vec2::new(at.x, at.y + size.y),
                            Vec2::new(size.x, max.y - at.y - size.y),
                            Vec2::new(uv_min.x, row_centre),
                            Vec2::new(uv_size.x, 0.0),
                        ));
                    }
                }
            }
        }
        out
    })
}

/// A raised layer's `z` as the order bits [`painter_order`] gives a
/// `z_order`.
#[inline]
fn raised_order(z: i32) -> u32 {
    z as u32 ^ 0x8000_0000
}

/// Moves `layer`'s batches to the end of `out`, every tile at `z_norm`.
fn place_layer(out: &mut Vec<SpriteBatch>, layer: &mut Vec<SpriteBatch>, z_norm: f32) {
    for mut batch in layer.drain(..) {
        for tile in &mut batch.instances {
            tile.z_norm = z_norm;
        }
        out.push(batch);
    }
}

/// The size render projects with: the surface size the app passed, else
/// `WindowSize`, which a pending resize leaves ahead of the surface (`D-114`).
pub(crate) fn render_viewport(world: &World) -> Option<(u32, u32)> {
    world
        .get_resource::<ExtractScratch>()
        .and_then(ExtractScratch::viewport)
        .or_else(|| {
            world
                .get_resource::<WindowSize>()
                .map(|size| (size.width, size.height))
        })
}

/// The world box the default extract culls by (redo 7.9): the camera's
/// visible box at the size render projects with. `None`, which culls
/// nothing, without a camera or a size. A game's own extract culls by it.
#[must_use]
pub fn view_box(world: &World) -> Option<(Vec2, Vec2)> {
    view_bounds(world)
}

/// The instance that draws `asset` over `size` from its top-left
/// `position`, turned by `rotation` about its centre, tinted `color`, at
/// `z_norm` (redo 7.9): the asset's UVs, as the default extract writes
/// them, for a game's own extract.
#[must_use]
pub fn sprite_instance(
    asset: &SpriteAsset,
    position: Vec2,
    size: Vec2,
    rotation: f32,
    color: [u8; 4],
    z_norm: f32,
) -> SpriteInstance {
    SpriteInstance {
        position: position.to_array(),
        size: size.to_array(),
        rotation,
        color,
        uv_min: asset.uv.min,
        uv_size: [
            asset.uv.max[0] - asset.uv.min[0],
            asset.uv.max[1] - asset.uv.min[1],
        ],
        z_norm,
        _pad: 0.0,
    }
}

/// Sprite batches built an instance at a time, for a game's own extract
/// (redo 7.9): an instance joins the last batch when it draws alike, on the
/// same page, filter and lighting, and opens one otherwise, so the batches
/// draw in push order.
#[derive(Default)]
pub struct Batches {
    batches: Vec<SpriteBatch>,
}

impl Batches {
    /// No batches yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Pushes `instance`, drawn from `asset`'s page with its filter and
    /// lighting.
    pub fn push(&mut self, asset: &SpriteAsset, instance: SpriteInstance) {
        let lit = asset.lit_atlas.is_some();
        match self.batches.last_mut() {
            Some(batch)
                if batch.texture == asset.atlas
                    && batch.filter == asset.filter
                    && batch.lit == lit =>
            {
                batch.instances.push(instance);
            }
            _ => {
                let mut batch = SpriteBatch::new(asset.atlas, asset.filter);
                batch.lit = lit;
                batch.instances.push(instance);
                self.batches.push(batch);
            }
        }
    }

    /// The batches, in push order.
    #[must_use]
    pub fn into_batches(self) -> Vec<SpriteBatch> {
        self.batches
    }
}

/// Most tiles one [`beam`] lays.
const BEAM_TILES: usize = 1 << 16;

/// Lays `asset` along the segment from `from` to `to`, `width` across (redo
/// 7.9): tiles of the sprite's own aspect at that width, turned along the
/// segment, their pattern moved `offset` world pixels toward `to`, so a
/// growing offset scrolls it; the tiles at the ends are cut short by their
/// UVs where the segment ends. A segment or width of 0 lays nothing.
#[allow(clippy::too_many_arguments)] // a beam's own description
pub fn beam(
    batches: &mut Batches,
    asset: &SpriteAsset,
    from: Vec2,
    to: Vec2,
    width: f32,
    offset: f32,
    color: [u8; 4],
    z_norm: f32,
) {
    let along = to - from;
    let length = along.length();
    if !(length > 0.0 && width > 0.0 && asset.width > 0 && asset.height > 0 && offset.is_finite()) {
        return;
    }
    let direction = along / length;
    let rotation = direction.y.atan2(direction.x);
    let tile = asset.width as f32 * width / asset.height as f32;
    let uv_min = Vec2::from(asset.uv.min);
    let uv_size = Vec2::from(asset.uv.max) - uv_min;
    let mut start = offset.rem_euclid(tile);
    if start > 0.0 {
        start -= tile;
    }
    for _ in 0..BEAM_TILES {
        if start >= length {
            break;
        }
        let (near, far) = (start.max(0.0), (start + tile).min(length));
        let (u0, u1) = ((near - start) / tile, (far - start) / tile);
        let centre = from + direction * (near + far) * 0.5;
        let size = Vec2::new(far - near, width);
        batches.push(
            asset,
            SpriteInstance {
                position: (centre - size * 0.5).to_array(),
                size: size.to_array(),
                rotation,
                color,
                uv_min: [uv_min.x + u0 * uv_size.x, uv_min.y],
                uv_size: [(u1 - u0) * uv_size.x, uv_size.y],
                z_norm,
                _pad: 0.0,
            },
        );
        start += tile;
    }
}

/// The world-space box render shows: the camera's visible box at
/// [`render_viewport`]. `None`, which culls nothing, without a camera or a
/// size.
fn view_bounds(world: &World) -> Option<(Vec2, Vec2)> {
    let camera = world.get_resource::<CameraState>()?;
    let (width, height) = render_viewport(world)?;
    Some(camera.visible_world_aabb(width as f32, height as f32))
}

/// Where an anchored sprite's quad starts from its entity's position (redo
/// 7.9): `R(c − a) − (c − a) − a`, `a` the anchor scaled with the sprite and
/// `c` the quad's centre. The anchor lands on the position, and the shader's
/// turn about the centre turns the quad about the anchor.
#[inline]
pub(crate) fn anchor_offset(anchor: [f32; 2], scale: Vec2, size: Vec2, rotation: f32) -> Vec2 {
    let anchor = Vec2::from(anchor) * scale;
    let arm = size * 0.5 - anchor;
    let turned = if rotation == 0.0 {
        arm
    } else {
        Vec2::from_angle(rotation).rotate(arm)
    };
    turned - arm - anchor
}

/// Whether a quad at top-left `position` with `size` (either sign) lies wholly
/// outside `view`. A rotated quad turns about its centre, so its bound is the
/// square of half-side `(|w| + |h|) / 2` there, which covers every angle. A
/// quad that touches the view is inside. Both bounds are computed and one is
/// selected: turned and unturned sprites mix within an archetype, and a branch
/// on the angle mispredicts.
#[inline]
fn outside_view(position: Vec2, size: Vec2, rotation: f32, (min, max): (Vec2, Vec2)) -> bool {
    let far = position + size;
    let centre = position + size * 0.5;
    let half = Vec2::splat(size.abs().element_sum() * 0.5);
    let turned = BVec2::splat(rotation != 0.0);
    let lo = Vec2::select(turned, centre - half, position.min(far));
    let hi = Vec2::select(turned, centre + half, position.max(far));
    (hi.cmplt(min) | lo.cmpgt(max)).any()
}

/// Pass 1's running state across archetypes.
struct Pass1 {
    last_class: Option<(BatchKey, u32)>,
    last_order: u64,
    in_painter_order: bool,
    set_bits: u64,
    common_bits: u64,
    culled: usize,
}

/// The buffers pass 1 writes.
struct Pass1Out<'a> {
    keys: &'a mut Vec<SortKey>,
    records: &'a mut Vec<SpriteInstance>,
    classes: &'a mut Vec<BatchClass>,
    class_of: &'a mut HashMap<BatchKey, u32>,
    warnings: &'a mut SpriteWarnings,
}

/// What a sprite ID and material resolve to in one archetype, kept while the
/// next row names the same pair.
#[derive(Clone, Copy)]
struct Resolved {
    id: SpriteAssetId,
    material: Option<MaterialAssetId>,
    class: u32,
    /// Whether the stock pipelines draw it, so the view culls it.
    culls: bool,
    base: Vec2,
    anchor: Option<[f32; 2]>,
    uv_min: [f32; 2],
    uv_size: [f32; 2],
}

/// Pass 1 over an archetype with no override block and no flip: the general
/// loop's keys and instances, row for row. Without `KEYED` it writes a key
/// for a culled sprite alone, holding the record count at its place.
#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn plain_rows<const KEYED: bool, P: Fn(usize, &Transform) -> Vec2>(
    st: &mut Pass1,
    out: &mut Pass1Out<'_>,
    assets: &AssetRegistry,
    view: Option<(Vec2, Vec2)>,
    unlit: bool,
    last_total: usize,
    (entities, transforms, sprites, visibilities): (
        &[Entity],
        &[Transform],
        &[Sprite],
        &[Visibility],
    ),
    place: P,
) {
    let mut last_class = st.last_class;
    let mut last_order = st.last_order;
    let mut in_painter_order = st.in_painter_order;
    let (mut set_bits, mut common_bits) = (st.set_bits, st.common_bits);
    let mut culled = st.culled;
    let mut memo: Option<Resolved> = None;
    let rows = entities
        .iter()
        .zip(transforms)
        .zip(sprites)
        .zip(visibilities)
        .enumerate();
    for (row, (((&e, t), s), v)) in rows {
        if !v.visible {
            continue;
        }
        let resolved = match memo {
            Some(kept) if kept.id == s.asset_id && kept.material == s.material_id => kept,
            _ => {
                let Some(asset) = assets.sprite(s.asset_id) else {
                    warn_once(out.warnings, assets, s.asset_id, SpriteWarning::NoAsset);
                    continue;
                };
                let lit = asset.lit_atlas.is_some() && !unlit;
                let effective_material = if lit { None } else { s.material_id };
                if lit && s.material_id.is_some() {
                    let warning = SpriteWarning::LitMaterial(s.material_id);
                    warn_once(out.warnings, assets, s.asset_id, warning);
                }
                let key: BatchKey = (asset.atlas.0, asset.filter, effective_material, None, lit);
                let class = match last_class {
                    Some((last_key, class)) if last_key == key => class,
                    _ => {
                        let classes = &mut *out.classes;
                        let class = *out.class_of.entry(key).or_insert_with(|| {
                            classes.push(BatchClass {
                                texture: asset.atlas,
                                filter: asset.filter,
                                material_id: effective_material,
                                uniform_overrides: None,
                                lit,
                                run: 0,
                                batch: 0,
                            });
                            (classes.len() - 1) as u32
                        });
                        last_class = Some((key, class));
                        class
                    }
                };
                let kept = Resolved {
                    id: s.asset_id,
                    material: s.material_id,
                    class,
                    culls: effective_material.is_none(),
                    base: Vec2::new(asset.width as f32, asset.height as f32),
                    anchor: asset.anchor,
                    uv_min: asset.uv.min,
                    uv_size: [
                        asset.uv.max[0] - asset.uv.min[0],
                        asset.uv.max[1] - asset.uv.min[1],
                    ],
                };
                memo = Some(kept);
                kept
            }
        };
        let mut position = place(row, t);
        let size = resolved.base * t.scale;
        if let Some(anchor) = resolved.anchor {
            position += anchor_offset(anchor, t.scale, size, t.rotation);
        }
        let order = painter_order(s.z_order, e.id());
        in_painter_order &= order >= last_order;
        last_order = order;
        set_bits |= order;
        common_bits &= order;
        // The sprite's place among those counted so far, culled or kept.
        let index = if KEYED {
            out.keys.len()
        } else {
            out.records.len() + culled
        };
        if resolved.culls && view.is_some_and(|view| outside_view(position, size, t.rotation, view))
        {
            culled += 1;
            out.keys.push(SortKey {
                order,
                record: if KEYED {
                    CULLED
                } else {
                    out.records.len() as u32
                },
                class: resolved.class,
            });
            continue;
        }
        if KEYED {
            out.keys.push(SortKey {
                order,
                record: out.records.len() as u32,
                class: resolved.class,
            });
        }
        out.records.push(SpriteInstance {
            position: [position.x, position.y],
            size: [size.x, size.y],
            rotation: t.rotation,
            color: s.color,
            uv_min: resolved.uv_min,
            uv_size: resolved.uv_size,
            z_norm: z_norm(last_total, index),
            _pad: 0.0,
        });
    }
    st.last_class = last_class;
    st.last_order = last_order;
    st.in_painter_order = in_painter_order;
    st.set_bits = set_bits;
    st.common_bits = common_bits;
    st.culled = culled;
}

fn extract_into(
    world: &World,
    assets: &AssetRegistry,
    view: Option<(Vec2, Vec2)>,
    buffers: &mut ExtractBuffers,
    raised: &mut [RaisedLayer],
) -> Vec<SpriteBatch> {
    #[cfg(test)]
    let (plain_loop, keyed_only) = (!buffers.general_only, buffers.keyed_only);
    #[cfg(not(test))]
    let (plain_loop, keyed_only) = (true, false);
    let ExtractBuffers {
        keys,
        keys_swap,
        records,
        classes,
        class_of,
        sprite_batch_lens,
        pool,
        batch_list,
        warnings,
        last_total: kept_total,
        one_batch,
        keyed_hold,
        #[cfg(test)]
        restarts,
        #[cfg(test)]
        keyless_frames,
        ..
    } = buffers;
    let camera_position = world
        .get_resource::<CameraState>()
        .map_or(Vec2::ZERO, |camera| camera.position);

    // Pass 1: an instance per visible sprite with a resolved asset that
    // culling keeps, in query order, and a key per such sprite, kept or
    // culled. The vectors keep their capacity across frames; when the last
    // frame's records left as a batch, the pool hands that vector back.
    let last_total = *kept_total;
    if records.capacity() == 0 {
        *records = pool.take(last_total);
    }
    // The frame after a one-batch frame starts keyless: only pass 2, the sort
    // and a count change read a kept sprite's key, and a frame that stays one
    // batch runs none of the first two. A keyless pass writes a key for a
    // culled sprite alone, and gives way to a keyed one, from the first
    // archetype, after the archetype that breaks the one batch.
    let mut keyed = keyed_only || !*one_batch || *keyed_hold > 0 || !raised.is_empty();
    *keyed_hold = keyed_hold.saturating_sub(1);
    let st = loop {
        keys.clear();
        records.clear();
        classes.clear();
        class_of.clear();
        let mut st = Pass1 {
            last_class: None,
            last_order: 0,
            in_painter_order: true,
            set_bits: 0,
            common_bits: u64::MAX,
            culled: 0,
        };
        let mut one_batch_so_far = true;
        // The slice form: each archetype's rows as its columns' slices. An
        // archetype with no override block and no flip, the common one,
        // takes the plain loop, which keeps what a sprite ID and material
        // resolve to while the pair repeats; the rest take the general one.
        let archetypes = world.query_slices::<(
            Entity,
            &Transform,
            &Sprite,
            &Visibility,
            Option<&UniformOverrideBlock>,
            Option<&ParallaxLayer>,
        )>();
        for (_, columns) in archetypes {
            let (entities, transforms, sprites, visibilities, override_blocks, parallaxes) =
                columns;
            let first = entities.first();
            let flips = first.is_some_and(|&e| world.has::<SpriteFlip>(e));
            let unlit = first.is_some_and(|&e| world.has::<Unlit>(e));
            if keyed {
                keys.reserve(entities.len());
            }
            if plain_loop && override_blocks.is_none() && !flips {
                let mut out = Pass1Out {
                    keys,
                    records,
                    classes,
                    class_of,
                    warnings,
                };
                let columns = (entities, transforms, sprites, visibilities);
                // Four loops: keyed or not, with a parallax column or none.
                macro_rules! rows {
                    ($keyed:literal, $place:expr) => {
                        plain_rows::<$keyed, _>(
                            &mut st, &mut out, assets, view, unlit, last_total, columns, $place,
                        )
                    };
                }
                match (keyed, parallaxes) {
                    (true, None) => rows!(true, |_, t| t.position),
                    (false, None) => rows!(false, |_, t| t.position),
                    (true, Some(layers)) => rows!(true, |row, t| {
                        parallax_world_position(
                            t.position,
                            layers[row].scroll_factor,
                            camera_position,
                        )
                    }),
                    (false, Some(layers)) => rows!(false, |row, t| {
                        parallax_world_position(
                            t.position,
                            layers[row].scroll_factor,
                            camera_position,
                        )
                    }),
                }
            } else {
                let rows = entities
                    .iter()
                    .zip(transforms)
                    .zip(sprites)
                    .zip(visibilities)
                    .zip(OptionalColumn(override_blocks.map(<[_]>::iter)))
                    .zip(OptionalColumn(parallaxes.map(<[_]>::iter)));
                rows.for_each(|(((((&e, t), s), v), override_block), parallax)| {
                    if !v.visible {
                        return;
                    }
                    let Some(asset) = assets.sprite(s.asset_id) else {
                        warn_once(warnings, assets, s.asset_id, SpriteWarning::NoAsset);
                        return;
                    };

                    let override_hash = override_block.map(override_key);
                    // M29: lit wins over material when both are present. The collision
                    // is intentionally a non-goal in M29 (`D-061`); warn once per
                    // sprite ID so debug logs surface the conflict rather than
                    // silently dropping the material. An unlit sprite draws on the
                    // stock pipeline, maps or not.
                    let lit = asset.lit_atlas.is_some() && !unlit;
                    let effective_material = if lit { None } else { s.material_id };
                    if lit && s.material_id.is_some() {
                        let warning = SpriteWarning::LitMaterial(s.material_id);
                        warn_once(warnings, assets, s.asset_id, warning);
                    }
                    let key: BatchKey = (
                        asset.atlas.0,
                        asset.filter,
                        effective_material,
                        override_hash,
                        lit,
                    );
                    let class = match st.last_class {
                        Some((last_key, class)) if last_key == key => class,
                        _ => {
                            let class = *class_of.entry(key).or_insert_with(|| {
                                classes.push(BatchClass {
                                    texture: asset.atlas,
                                    filter: asset.filter,
                                    material_id: effective_material,
                                    uniform_overrides: override_block.copied(),
                                    lit,
                                    run: 0,
                                    batch: 0,
                                });
                                (classes.len() - 1) as u32
                            });
                            st.last_class = Some((key, class));
                            class
                        }
                    };

                    let mut position = match parallax {
                        Some(layer) => parallax_world_position(
                            t.position,
                            layer.scroll_factor,
                            camera_position,
                        ),
                        None => t.position,
                    };
                    let size = Vec2::new(
                        asset.width as f32 * t.scale.x,
                        asset.height as f32 * t.scale.y,
                    );
                    let flip = flips && world.get::<SpriteFlip>(e).is_some_and(|flip| flip.0);
                    if let Some(anchor) = asset.anchor {
                        // A flipped sprite mirrors about its anchor: the mirrored
                        // image's anchor lands on the entity.
                        let anchor = if flip {
                            [asset.width as f32 - anchor[0], anchor[1]]
                        } else {
                            anchor
                        };
                        position += anchor_offset(anchor, t.scale, size, t.rotation);
                    }
                    let order = painter_order(s.z_order, e.id());
                    st.in_painter_order &= order >= st.last_order;
                    st.last_order = order;
                    st.set_bits |= order;
                    st.common_bits &= order;
                    let index = if keyed {
                        keys.len()
                    } else {
                        records.len() + st.culled
                    };
                    if effective_material.is_none()
                        && view.is_some_and(|view| outside_view(position, size, t.rotation, view))
                    {
                        st.culled += 1;
                        keys.push(SortKey {
                            order,
                            record: if keyed { CULLED } else { records.len() as u32 },
                            class,
                        });
                        return;
                    }
                    if keyed {
                        keys.push(SortKey {
                            order,
                            record: records.len() as u32,
                            class,
                        });
                    }
                    let mut uv_min = asset.uv.min;
                    let mut uv_size = [
                        asset.uv.max[0] - asset.uv.min[0],
                        asset.uv.max[1] - asset.uv.min[1],
                    ];
                    if flip {
                        // A negative width samples right to left.
                        uv_min[0] = asset.uv.max[0];
                        uv_size[0] = -uv_size[0];
                    }
                    records.push(SpriteInstance {
                        position: [position.x, position.y],
                        size: [size.x, size.y],
                        rotation: t.rotation,
                        color: s.color,
                        uv_min,
                        uv_size,
                        // Right for the single-batch case below when the count
                        // repeats last frame's; rewritten otherwise.
                        z_norm: z_norm(last_total, index),
                        _pad: 0.0,
                    });
                });
            }
            // Keyless so far, the frame must still be the one batch the
            // single-batch path below takes: in painter order, one class,
            // one `z_order`.
            if !keyed
                && (!st.in_painter_order
                    || classes.len() > 1
                    || (!classes.is_empty() && (st.set_bits ^ st.common_bits) >> 32 != 0))
            {
                one_batch_so_far = false;
                break;
            }
        }
        if one_batch_so_far {
            break st;
        }
        keyed = true;
        *keyed_hold = KEYED_HOLD;
        #[cfg(test)]
        {
            *restarts += 1;
        }
    };
    let Pass1 {
        in_painter_order,
        set_bits,
        common_bits,
        culled,
        ..
    } = st;

    // Bits of the order that differ between sprites. The orders are distinct,
    // so any correct sort gives the one painter order a stable sort by
    // `(z_order, entity id)` gave.
    let varying_bits = set_bits ^ common_bits;
    if !in_painter_order {
        sort_keys(keys, keys_swap, varying_bits);
    }

    // A keyless pass wrote a key for each culled sprite alone, and ended
    // only as the one batch.
    let total = if keyed {
        keys.len()
    } else {
        records.len() + culled
    };
    let single = total > 0
        && (!keyed
            || (in_painter_order
                && classes.len() == 1
                && varying_bits >> 32 == 0
                && raised.is_empty()));
    *kept_total = total;
    *one_batch = single;
    #[cfg(test)]
    {
        *keyless_frames += u32::from(!keyed);
    }
    let mut out = std::mem::take(batch_list);
    if single {
        // One class in one z-run, already in painter order: the records are
        // the batch's instances, and their `z_norm` stands when pass 1
        // assumed the right count. The keys are in query order, so a kept
        // key's record sits at its slot.
        let mut instances = std::mem::take(records);
        if total != last_total && keyed {
            for (idx_in_order, key) in keys.iter().enumerate() {
                if key.record != CULLED {
                    instances[key.record as usize].z_norm = z_norm(total, idx_in_order);
                }
            }
        } else if total != last_total {
            // Keyless, an instance's place is its slot plus the culled
            // sprites before it, each of which left the record count at its
            // place.
            let mut culls = keys.iter().map(|key| key.record as usize).peekable();
            let mut culled_before = 0;
            for (slot, instance) in instances.iter_mut().enumerate() {
                while culls.next_if(|&at| at <= slot).is_some() {
                    culled_before += 1;
                }
                instance.z_norm = z_norm(total, slot + culled_before);
            }
        }
        if instances.is_empty() {
            *records = instances;
        } else {
            let class = &classes[0];
            out.push(SpriteBatch {
                texture: class.texture,
                filter: class.filter,
                instances,
                material_id: class.material_id,
                uniform_overrides: class.uniform_overrides,
                lit: class.lit,
            });
        }
    } else {
        // Pass 2: batch by effective material state inside each z-run. A
        // class opens a new batch in every z-run it appears in, at its first
        // sprite, culled or not; a span of sprites sharing z and class goes in
        // at once. A batch takes its instance vector at its first kept sprite.
        // A raised layer goes in before the first z-run at or above its `z`,
        // one place of `places` in the depth order.
        let places = total + raised.len();
        let mut layers_placed = 0;
        let mut run = 0u32;
        let mut run_z: Option<u32> = None;
        let mut start = 0;
        while start < total {
            let first = keys[start];
            let z = (first.order >> 32) as u32;
            let same_span =
                |key: &&SortKey| key.class == first.class && (key.order >> 32) as u32 == z;
            // Culled keys of the span, counted only on frames that cull.
            let mut span_culled = 0;
            let end = start
                + if culled == 0 {
                    keys[start..].iter().take_while(same_span).count()
                } else {
                    keys[start..]
                        .iter()
                        .take_while(same_span)
                        .inspect(|key| span_culled += usize::from(key.record == CULLED))
                        .count()
                };
            if run_z != Some(z) {
                run_z = Some(z);
                run += 1;
                while let Some((layer_z, layer)) = raised.get_mut(layers_placed)
                    && raised_order(*layer_z) <= z
                {
                    place_layer(&mut out, layer, z_norm(places, start + layers_placed));
                    layers_placed += 1;
                }
            }
            let class = &mut classes[first.class as usize];
            if class.run != run {
                class.run = run;
                class.batch = out.len() as u32;
                out.push(SpriteBatch {
                    texture: class.texture,
                    filter: class.filter,
                    instances: Vec::new(),
                    material_id: class.material_id,
                    uniform_overrides: class.uniform_overrides,
                    lit: class.lit,
                });
            }
            let batch = class.batch as usize;
            let span = &keys[start..end];
            if span_culled < span.len() {
                let instances = &mut out[batch].instances;
                if instances.capacity() == 0 {
                    let len = sprite_batch_lens.get(batch).map_or(0, |&len| len as usize);
                    *instances = pool.take(len);
                }
                let first_place = start + layers_placed;
                let gather = |(offset, key): (usize, &SortKey)| {
                    let mut instance = records[key.record as usize];
                    instance.z_norm = z_norm(places, first_place + offset);
                    instance
                };
                if span_culled == 0 {
                    instances.extend(span.iter().enumerate().map(gather));
                } else {
                    instances.extend(
                        span.iter()
                            .enumerate()
                            .filter(|(_, key)| key.record != CULLED)
                            .map(gather),
                    );
                }
            }
            start = end;
        }
        while let Some((_, layer)) = raised.get_mut(layers_placed) {
            place_layer(&mut out, layer, z_norm(places, total + layers_placed));
            layers_placed += 1;
        }
        if culled > 0 {
            out.retain(|batch| !batch.instances.is_empty());
        }
    }

    sprite_batch_lens.clear();
    sprite_batch_lens.extend(out.iter().map(|batch| batch.instances.len() as u32));
    out
}

/// Below this many keys a comparison sort beats the radix passes.
const RADIX_SORT_MIN: usize = 1024;

/// Sorts `keys` by `order`. Large inputs take a least-significant-digit radix
/// sort over the bytes named in `varying_bits`, whose cost does not depend on
/// how shuffled the keys arrive.
fn sort_keys(keys: &mut Vec<SortKey>, swap: &mut Vec<SortKey>, varying_bits: u64) {
    if keys.len() < RADIX_SORT_MIN {
        keys.sort_unstable_by_key(|key| key.order);
        return;
    }
    // Every pass overwrites all of `swap`, so its old content can stay.
    swap.resize(
        keys.len(),
        SortKey {
            order: 0,
            record: 0,
            class: 0,
        },
    );
    for shift in (0..u64::BITS).step_by(8) {
        // A byte no two orders differ in needs no pass.
        if (varying_bits >> shift) as u8 == 0 {
            continue;
        }
        let digit = |key: &SortKey| ((key.order >> shift) & 0xff) as usize;
        let mut starts = [0usize; 256];
        for key in keys.iter() {
            starts[digit(key)] += 1;
        }
        let mut next = 0;
        for start in &mut starts {
            let count = *start;
            *start = next;
            next += count;
        }
        for key in keys.iter() {
            let slot = &mut starts[digit(key)];
            swap[*slot] = *key;
            *slot += 1;
        }
        std::mem::swap(keys, swap);
    }
}

#[cfg(test)]
#[path = "tests/sprite_extract.rs"]
mod tests;
