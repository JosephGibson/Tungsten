//! Default sprite extract: `Transform + Sprite + Visibility` -> [`SpriteBatch`].
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
//! pass: its instance list becomes the batch. Asset IDs resolve through a
//! last-seen memo and a small direct-mapped cache before the registry's map.
//! With [`ExtractScratch`] in the world every buffer, the batches' instance
//! vectors included, is reused across frames. Output bytes are unchanged from
//! the tuple-sorting extract this replaced; `tests/sprite_extract.rs` keeps
//! that extract as the reference.

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::Hasher;

use glam::Vec2;
use tungsten_core::assets::TextureHandle;
use tungsten_core::tween::UniformOverrideBlock;
use tungsten_core::{
    AssetRegistry, CameraState, FilterMode, MaterialAssetId, ParallaxLayer, Sprite, SpriteAsset,
    Transform, Visibility, World, parallax_world_position,
};
use tungsten_render::{SpriteBatch, SpriteInstance};

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
    record: u32,
    class: u32,
}

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
}

/// World resource that lets [`extract_sprites_default`] and
/// [`extract_tilemaps`](crate::extract_tilemaps) reuse their buffers across
/// frames. `App` inserts it and hands each frame's batches back after the
/// render stage; without it both extracts allocate per call.
#[derive(Default)]
pub(crate) struct ExtractScratch(RefCell<ExtractBuffers>);

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

    /// Takes a drawn frame's batches back: their instance vectors serve the
    /// next frame's batches.
    pub(crate) fn recycle(&self, mut batches: Vec<SpriteBatch>) {
        let mut buffers = self.0.borrow_mut();
        buffers.pool.refill(&mut batches);
        buffers.batch_list = batches;
    }
}

const ASSET_CACHE_SLOTS: usize = 64;

/// Sprite lookups of one extract call: the last ID seen, then a direct-mapped
/// cache keyed by a cheap hash of the ID and verified by comparing it, then
/// the registry.
struct AssetLookup<'w> {
    assets: &'w AssetRegistry,
    last: Option<(&'w str, &'w SpriteAsset)>,
    slots: [Option<(&'w str, &'w SpriteAsset)>; ASSET_CACHE_SLOTS],
}

impl<'w> AssetLookup<'w> {
    fn new(assets: &'w AssetRegistry) -> Self {
        Self {
            assets,
            last: None,
            slots: [None; ASSET_CACHE_SLOTS],
        }
    }

    #[inline]
    fn get(&mut self, id: &'w str) -> Option<&'w SpriteAsset> {
        if let Some((last_id, asset)) = self.last
            && last_id == id
        {
            return Some(asset);
        }
        let slot = &mut self.slots[asset_cache_slot(id)];
        let asset = match *slot {
            Some((cached_id, asset)) if cached_id == id => asset,
            _ => {
                let asset = self.assets.get_sprite(id)?;
                *slot = Some((id, asset));
                asset
            }
        };
        self.last = Some((id, asset));
        Some(asset)
    }
}

/// Cache slot of a sprite ID from its length and its first and last eight
/// bytes. Collisions only cost a registry lookup.
#[inline]
fn asset_cache_slot(id: &str) -> usize {
    let bytes = id.as_bytes();
    let len = bytes.len();
    let (head, tail) = if len >= 8 {
        (
            u64::from_le_bytes(bytes[..8].try_into().unwrap()),
            u64::from_le_bytes(bytes[len - 8..].try_into().unwrap()),
        )
    } else {
        let mut word = [0u8; 8];
        word[..len].copy_from_slice(bytes);
        let word = u64::from_le_bytes(word);
        (word, word)
    };
    let mixed = (head ^ tail.rotate_left(29) ^ len as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    (mixed >> (u64::BITS - ASSET_CACHE_SLOTS.ilog2())) as usize
}

/// Default sprite extract.
#[must_use]
pub fn extract_sprites_default(world: &World) -> Vec<SpriteBatch> {
    let Some(assets) = world.get_resource::<AssetRegistry>() else {
        return Vec::new();
    };
    ExtractScratch::with(world, |buffers| extract_into(world, assets, buffers))
}

fn extract_into(
    world: &World,
    assets: &AssetRegistry,
    buffers: &mut ExtractBuffers,
) -> Vec<SpriteBatch> {
    let ExtractBuffers {
        keys,
        keys_swap,
        records,
        classes,
        class_of,
        sprite_batch_lens,
        pool,
        batch_list,
        ..
    } = buffers;
    let camera_position = world
        .get_resource::<CameraState>()
        .map_or(Vec2::ZERO, |camera| camera.position);

    // Pass 1: one key and one instance per visible sprite with a resolved
    // asset, in query order. The vectors keep their capacity across frames;
    // when the last frame's records left as a batch, the pool hands that
    // vector back.
    let last_total = keys.len();
    keys.clear();
    records.clear();
    if records.capacity() == 0 {
        *records = pool.take(last_total);
    }
    classes.clear();
    class_of.clear();
    let mut lookup = AssetLookup::new(assets);
    let mut last_class: Option<(BatchKey, u32)> = None;
    let mut last_order = 0u64;
    let mut in_painter_order = true;
    let (mut set_bits, mut common_bits) = (0u64, u64::MAX);
    world
        .query3_opt2::<Transform, Sprite, Visibility, UniformOverrideBlock, ParallaxLayer>()
        .for_each(|(e, t, s, v, override_block, parallax)| {
            if !v.visible {
                return;
            }
            let Some(asset) = lookup.get(&s.asset_id) else {
                return;
            };

            let override_hash = override_block.map(override_key);
            // M29: lit wins over material when both are present. The collision
            // is intentionally a non-goal in M29 (see plan); warn so debug logs
            // surface the conflict rather than silently dropping the material.
            let lit = asset.lit_atlas.is_some();
            let effective_material = if lit { None } else { s.material_id };
            if lit && s.material_id.is_some() {
                log::warn!(
                    "lit sprite '{}' carries material_id {:?}; lit wins (material UBO not bound)",
                    s.asset_id,
                    s.material_id
                );
            }
            let key: BatchKey = (
                asset.atlas.0,
                asset.filter,
                effective_material,
                override_hash,
                lit,
            );
            let class = match last_class {
                Some((last_key, class)) if last_key == key => class,
                _ => {
                    let class = *class_of.entry(key).or_insert_with(|| {
                        classes.push(BatchClass {
                            texture: asset.atlas,
                            filter: asset.filter,
                            material_id: effective_material,
                            uniform_overrides: if lit { None } else { override_block.copied() },
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

            let position = match parallax {
                Some(layer) => {
                    parallax_world_position(t.position, layer.scroll_factor, camera_position)
                }
                None => t.position,
            };
            let order = painter_order(s.z_order, e.id());
            in_painter_order &= order >= last_order;
            last_order = order;
            set_bits |= order;
            common_bits &= order;
            keys.push(SortKey {
                order,
                record: records.len() as u32,
                class,
            });
            records.push(SpriteInstance {
                position: [position.x, position.y],
                size: [
                    asset.width as f32 * t.scale.x,
                    asset.height as f32 * t.scale.y,
                ],
                rotation: t.rotation,
                color: s.color,
                uv_min: asset.uv.min,
                uv_size: [
                    asset.uv.max[0] - asset.uv.min[0],
                    asset.uv.max[1] - asset.uv.min[1],
                ],
                // Right for the single-batch case below when the count
                // repeats last frame's; rewritten otherwise.
                z_norm: z_norm(last_total, records.len()),
                _pad: 0.0,
            });
        });

    // Bits of the order that differ between sprites. The orders are distinct,
    // so any correct sort gives the one painter order a stable sort by
    // `(z_order, entity id)` gave.
    let varying_bits = set_bits ^ common_bits;
    if !in_painter_order {
        sort_keys(keys, keys_swap, varying_bits);
    }

    let total = keys.len();
    let mut out = std::mem::take(batch_list);
    if total > 0 && in_painter_order && classes.len() == 1 && varying_bits >> 32 == 0 {
        // One class in one z-run, already in painter order: the records are
        // the batch's instances, and their `z_norm` stands when pass 1
        // assumed the right count.
        let mut instances = std::mem::take(records);
        if total != last_total {
            for (idx_in_order, instance) in instances.iter_mut().enumerate() {
                instance.z_norm = z_norm(total, idx_in_order);
            }
        }
        let class = &classes[0];
        out.push(SpriteBatch {
            texture: class.texture,
            filter: class.filter,
            instances,
            material_id: class.material_id,
            uniform_overrides: class.uniform_overrides,
            lit: class.lit,
        });
    } else {
        // Pass 2: batch by effective material state inside each z-run. A
        // class opens a new batch in every z-run it appears in, at its first
        // sprite; a span of sprites sharing z and class goes in at once.
        let mut run = 0u32;
        let mut run_z: Option<u32> = None;
        let mut start = 0;
        while start < total {
            let first = keys[start];
            let z = (first.order >> 32) as u32;
            let end = start
                + keys[start..]
                    .iter()
                    .take_while(|key| key.class == first.class && (key.order >> 32) as u32 == z)
                    .count();
            if run_z != Some(z) {
                run_z = Some(z);
                run += 1;
            }
            let class = &mut classes[first.class as usize];
            if class.run != run {
                class.run = run;
                class.batch = out.len() as u32;
                let len = sprite_batch_lens
                    .get(out.len())
                    .map_or(0, |&len| len as usize);
                out.push(SpriteBatch {
                    texture: class.texture,
                    filter: class.filter,
                    instances: pool.take(len),
                    material_id: class.material_id,
                    uniform_overrides: class.uniform_overrides,
                    lit: class.lit,
                });
            }
            out[class.batch as usize]
                .instances
                .extend(keys[start..end].iter().enumerate().map(|(offset, key)| {
                    let mut instance = records[key.record as usize];
                    instance.z_norm = z_norm(total, start + offset);
                    instance
                }));
            start = end;
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
