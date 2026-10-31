//! Once-per-frame gather of entity and tilemap collider proxies in
//! deterministic `World` order (D-066), their warm-start keys and their
//! layers; sensors are gathered beside the proxies, never as one. Tile
//! proxies come from a cache of the collision layers that asks the registry
//! every frame whether a map changed (its cursor) and re-traces only the
//! 16 × 16 chunks around the cells written (`outline`); each is in its
//! collision layer's physics layers and takes its tile's physics material's
//! restitution and friction, resolved when a map is traced and again when
//! the `PhysicsMaterials` resource's version moves. The cache keeps the
//! outline's runs beside them, which casts meet.

use super::outline::{CHUNK, LayerCells, Run, TileClass, trace_chunk};
use super::{Layers, NO_SPIN, Proxy, ProxyLayers, SensorBody, SpinState, StepExtra, SweptSensor};
use crate::assets::{
    LayerKind, MapCursor, TileChanges, TileIndex, TilemapData, TilemapInstance, TilemapRegistry,
};
use crate::ecs::{Entity, World};
use crate::physics::collision::FACE_ALL;
#[cfg(test)]
use crate::physics::collision::{FACE_BOTTOM, FACE_LEFT, FACE_RIGHT, FACE_TOP};
use crate::physics::components::{BodyKind, Collider, Position, RigidBody, Shape, Spin, Velocity};
use crate::physics::query::TileHit;
use crate::physics::{PhysicsMaterial, PhysicsMaterials};
use glam::Vec2;
use std::ops::RangeInclusive;

/// Warm-start identity for entity proxies; tag bit 63 separates the entity
/// keyspace from tile hashes.
pub(super) fn entity_key(entity: Entity) -> u64 {
    (1 << 63) | (u64::from(entity.generation & 0x7FFF_FFFF) << 32) | u64::from(entity.index)
}

/// The entity an [`entity_key`] names, its generation's low 31 bits kept.
pub(super) fn key_entity(key: u64) -> Entity {
    Entity {
        index: key as u32,
        generation: ((key >> 32) & 0x7FFF_FFFF) as u32,
    }
}

/// Warm-start identity for tile proxies, hashed from the tile center; stable
/// while the tilemap does not move (a moved map only loses warm starts).
fn tile_key(center: Vec2) -> u64 {
    let h = u64::from(center.x.to_bits()).wrapping_mul(0x8DA6_B343_D816_3841)
        ^ u64::from(center.y.to_bits()).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let h = h ^ (h >> 31);
    h & !(1 << 63)
}

/// A tile proxy's warm-start identity: [`tile_key`], a one-way cell's with
/// its flag mixed in and a slope cell's with its kind, so a cell turning
/// one-way, another slope or back is another support to the sleep record,
/// which wakes the sleepers on it (`D-156`).
fn cell_key(center: Vec2, one_way: bool, slope: u8) -> u64 {
    let mut key = tile_key(center);
    if one_way {
        let h = (key ^ 0x6F6E_655F_7761_7921).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        key = (h ^ (h >> 31)) & !(1 << 63);
    }
    if slope != 0 {
        let h =
            (key ^ (0x736C_6F70_6500_0000 | u64::from(slope))).wrapping_mul(0x94D0_49BB_1331_11EB);
        key = (h ^ (h >> 29)) & !(1 << 63);
    }
    key
}

/// A tile's identity in sensor overlap keys: its map entity, layer and
/// cell, so a map whose origin moves keeps it, and another layer's tile at
/// the cell is another tile. Bit 63 stays clear, apart from entity keys.
fn tile_identity(map: Entity, layer: usize, col: u32, row: u32) -> u64 {
    let cell = (u64::from(row) << 32) | u64::from(col);
    let h = entity_key(map).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (layer as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ cell.wrapping_mul(0x8DA6_B343_D816_3841);
    let h = (h ^ (h >> 29)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    (h ^ (h >> 32)) & !(1 << 63)
}

/// A run's identity in the spatial index: a mix of its map, layer, first
/// and last cell and mask. Bit 63 stays clear, apart from entity keys.
fn run_key(map: Entity, layer: usize, run: &Run) -> u64 {
    let last = (u64::from(run.row + run.rows - 1) << 32) | u64::from(run.col + run.cols - 1);
    let h = tile_identity(map, layer, run.col, run.row).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ last.wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ u64::from(run.mask).wrapping_mul(0x8DA6_B343_D816_3841);
    let h = (h ^ (h >> 31)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    (h ^ (h >> 29)) & !(1 << 63)
}

/// Gather entity + tilemap proxies in deterministic `World` order, once per
/// frame (D-066); the gather runs before any integration, so
/// `prev_center == center` at gather time. Each proxy's layers go to
/// `layers`, in proxy order, which notes whether any proxy or sensor is off
/// the defaults. A sensor row joins `sensors` instead of the proxies; in a
/// frame with sensors, `tile_ids` gets each tile proxy's identity, in proxy
/// order, and stays empty otherwise. The tile proxies are `tiles`', brought
/// up to the World's maps first. `extra` gets each entity proxy's gravity
/// scale once a dynamic body's differs from 1, 1 for every proxy that is
/// not dynamic, and each proxy's friction, the tiles' included, once one is
/// above 0; and each dynamic circle that rolls and has a [`Spin`] its turn,
/// with every proxy's rolling resistance once one is above 0. Returns how many
/// kinematic proxies it staged, which sets the frame's substep count.
pub(super) fn gather_proxies(
    world: &World,
    proxies: &mut Vec<Proxy>,
    layers: &mut ProxyLayers,
    sensors: &mut Vec<SensorBody>,
    tile_ids: &mut Vec<u64>,
    tiles: &mut TileCache,
    extra: &mut StepExtra,
) -> usize {
    proxies.clear();
    layers.of.clear();
    sensors.clear();
    tile_ids.clear();
    extra.gravity_scales.clear();
    extra.frictions.clear();
    extra.spin_of.clear();
    extra.spins.clear();
    extra.rollings.clear();
    extra.swept.clear();
    let mut kinematics = 0;
    let mut layered = false;

    // Columnar 4-way gather: RigidBody/Velocity columns resolve once per
    // archetype — no per-entity random lookups (D-066).
    for (entity, collider, position, body, velocity) in world.query::<(
        Entity,
        &Collider,
        &Position,
        Option<&RigidBody>,
        Option<&Velocity>,
    )>() {
        let has_velocity = velocity.is_some();
        let velocity = velocity.map_or(Vec2::ZERO, |v| v.0);
        let body = body.copied();
        let body_layers = body.as_ref().map_or(Layers::DEFAULT, Layers::of);
        layered |= body_layers != Layers::DEFAULT;
        if body.is_some_and(|b| b.kind == BodyKind::Sensor) {
            if body.is_some_and(|b| b.swept) {
                extra.swept.push(SweptSensor {
                    sensor: sensors.len() as u32,
                    start: position.0 + collider.offset,
                });
            }
            sensors.push(SensorBody {
                entity,
                key: entity_key(entity),
                center: position.0 + collider.offset,
                shape: collider.shape,
                velocity,
                layers: body_layers,
            });
            continue;
        }
        // Every kind but `Dynamic` is immovable: inverse mass 0.
        let (is_dynamic, kinematic, inv_mass, restitution) = match body {
            Some(b) if b.kind == BodyKind::Dynamic => (true, false, b.inv_mass, b.restitution),
            Some(b) if b.kind == BodyKind::Kinematic => {
                kinematics += 1;
                (false, true, 0.0, b.restitution)
            }
            Some(b) => (false, false, 0.0, b.restitution),
            None => (false, false, 0.0, 0.0),
        };
        let center = position.0 + collider.offset;
        let gravity_scale = match body {
            Some(b) if is_dynamic => b.gravity_scale,
            _ => 1.0,
        };
        // Once one scale differs from 1, every proxy has one; once one
        // friction is above 0, likewise.
        let scales = &mut extra.gravity_scales;
        if gravity_scale != 1.0 || !scales.is_empty() {
            scales.resize(proxies.len(), 1.0);
            scales.push(gravity_scale);
        }
        let friction = body.map_or(0.0, |b| b.friction);
        let frictions = &mut extra.frictions;
        if friction != 0.0 || !frictions.is_empty() {
            frictions.resize(proxies.len(), 0.0);
            frictions.push(friction);
        }
        let rolling = body.map_or(0.0, |b| b.rolling);
        let rollings = &mut extra.rollings;
        if rolling != 0.0 || !rollings.is_empty() {
            rollings.resize(proxies.len(), 0.0);
            rollings.push(rolling);
        }
        // A dynamic circle that rolls turns; its `Spin` is read below, so
        // a world where nothing rolls never reads one.
        if let (Some(b), Shape::Circle { radius }) = (body, collider.shape)
            && b.rolls
            && is_dynamic
            && radius > 0.0
        {
            extra.spins.push(SpinState {
                proxy: proxies.len() as u32,
                entity,
                angle: 0.0,
                angular_velocity: 0.0,
                radius,
            });
        }
        proxies.push(Proxy {
            entity: Some(entity),
            key: entity_key(entity),
            center,
            prev_center: center,
            velocity,
            offset: collider.offset,
            shape: collider.shape,
            is_dynamic,
            has_velocity,
            inv_mass,
            restitution,
            face_mask: FACE_ALL,
            slope: 0,
            sleeping: false,
            kinematic,
            kept_awake: false,
        });
        layers.of.push(body_layers);
    }

    tiles.refresh(world);
    let bodies = proxies.len();
    // The tiles' frictions and rolling resistances follow the bodies'; the
    // rolling resistances matter only while something spins.
    let fill = |values: &mut Vec<f32>, tile_values: &[f32]| {
        if !(values.is_empty() && tile_values.is_empty()) {
            values.resize(bodies, 0.0);
            if tile_values.is_empty() {
                values.resize(bodies + tiles.len(), 0.0);
            } else {
                values.extend_from_slice(tile_values);
            }
        }
    };
    fill(&mut extra.frictions, tiles.frictions());
    // Each roller's turn, those with a `Spin`; then every proxy's slot.
    extra.spins.retain_mut(|turn| {
        let Some(spin) = world.get::<Spin>(turn.entity) else {
            return false;
        };
        turn.angle = spin.angle;
        turn.angular_velocity = spin.angular_velocity;
        true
    });
    if extra.spins.is_empty() {
        extra.rollings.clear();
    } else {
        fill(&mut extra.rollings, tiles.rollings());
        extra.spin_of.resize(bodies + tiles.len(), NO_SPIN);
        for (slot, turn) in extra.spins.iter().enumerate() {
            extra.spin_of[turn.proxy as usize] = slot as u32;
        }
    }
    proxies.extend_from_slice(&tiles.proxies);
    layers.of.extend_from_slice(&tiles.layers);
    layers.layered = layered || tiles.state.layered;
    if !sensors.is_empty() {
        tile_ids.extend_from_slice(&tiles.ids);
    }
    kinematics
}

/// The static tile proxies of every collision layer, each in its layer's
/// physics layers, in gather order: each map's, in query order, one per
/// solid cell in layer, row and column order, with each one's sensor
/// identity, its face mask the outline's; the outline's runs, which casts
/// meet; and each collision layer's cells as the last refresh read them,
/// which region queries and sensors read. Per map, layer and 16 × 16 chunk
/// the cache keeps the masks and runs traced from those cells. A map is
/// traced again whole when it appears, moves, names another map or is
/// replaced; a write to its collision cells re-traces the chunks holding
/// each written cell and its four neighbours, and its proxies are rebuilt
/// from its masks; one that went drops out. So the proxies, their order,
/// keys, face masks, layers and identities, and the runs, are what a scan
/// of this frame would give.
#[derive(Debug, Default)]
pub(in crate::physics) struct TileCache {
    /// The tile proxies, in gather order.
    pub(super) proxies: Vec<Proxy>,
    /// Each tile proxy's layers and mask, in the same order.
    pub(super) layers: Vec<Layers>,
    /// Each tile proxy's identity in sensor overlap keys, in the same order.
    pub(super) ids: Vec<u64>,
    /// The rest, which no frame copies: boxed, so the step's buffers, which
    /// it moves out of the World and back each step, stay small.
    state: Box<CacheState>,
}

/// The tile cache's maps and bookkeeping beside the arrays the gather
/// copies.
#[derive(Debug, Default)]
struct CacheState {
    /// Bumped by every change to the proxies; 0 before the first.
    version: u64,
    maps: Vec<CachedMap>,
    /// Some tile proxy is off [`Layers::DEFAULT`].
    layered: bool,
    /// The outline's runs, in their first cell's tile order: by map, layer,
    /// row and column.
    runs: Vec<TileRun>,
    /// The maps whose every cell the last refresh changed: replaced,
    /// added, moved, resized or gone.
    replaced: Vec<Entity>,
    /// The collision cells the last refresh found written in the other
    /// maps, by map in cache order, each map's sorted by layer, row and
    /// column.
    written: Vec<(Entity, Vec<WrittenCell>)>,
    /// Those cells' sensor identities, sorted.
    written_ids: Vec<u64>,
    /// Chunks traced since the cache was made.
    traced: u64,
    /// Each tile proxy's friction, in gather order; empty while every
    /// tile's is 0.
    frictions: Vec<f32>,
    /// Each tile proxy's rolling resistance, likewise.
    rollings: Vec<f32>,
    /// The version of the World's `PhysicsMaterials` the maps' coefficients
    /// were resolved against; `None` without the resource.
    materials_version: Option<u64>,
    /// The arrays the last refresh replaced, kept for the next one to fill.
    spare: TileArrays,
    /// Maps traced, whole or in part, since the cache was made.
    #[cfg(test)]
    scans: usize,
}

/// One run of the outline as the index holds it: its box, exposed faces,
/// layers and identity, and its cells by its map's and layer's places in
/// the cache: from its first, `cols` across and `rows` down.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::physics) struct TileRun {
    pub(in crate::physics) center: Vec2,
    pub(in crate::physics) half: Vec2,
    /// Its cells' mask, the one-way flag included.
    pub(in crate::physics) face_mask: u8,
    /// Its layer's layers and mask.
    pub(in crate::physics) layers: u32,
    pub(in crate::physics) mask: u32,
    pub(in crate::physics) identity: u64,
    pub(in crate::physics) map_slot: u32,
    pub(in crate::physics) layer_slot: u32,
    pub(in crate::physics) col: u32,
    pub(in crate::physics) row: u32,
    pub(in crate::physics) cols: u32,
    pub(in crate::physics) rows: u32,
}

/// One map as its last refresh read it: where its proxies sit, and its
/// collision layers' cells where its instance placed them, with their
/// masks and runs.
#[derive(Debug)]
struct CachedMap {
    entity: Entity,
    id: String,
    origin: [u32; 2],
    cursor: MapCursor,
    /// Its proxies' first index in the cache's arrays, and their count.
    start: usize,
    len: usize,
    /// The instance's origin and the tile size, in world pixels.
    at: Vec2,
    tile: Vec2,
    width: u32,
    height: u32,
    /// By tileset entry: what a tile breaks runs on.
    classes: Vec<TileClass>,
    /// Its collision layers, in layer order.
    layers: Vec<CachedLayer>,
}

/// A collision layer's cells as the cache's last refresh read them, and
/// what their trace gave.
#[derive(Debug)]
struct CachedLayer {
    /// Its index in the map's layers.
    index: usize,
    layers: Layers,
    tiles: Vec<TileIndex>,
    /// Each cell's exposed faces, row-major: 0 for an empty or interior one.
    masks: Vec<u8>,
    /// By chunk, row-major: its runs in their first cell's row-major order.
    chunks: Vec<Vec<Run>>,
}

/// A solid cell of a collision layer, as region queries, sensors and the
/// cells of a cast's runs test it: a box exactly as its proxy's is, with
/// its layers, sensor identity and cell, and its map's and layer's places
/// in the cache, which rank it in tile order.
#[derive(Debug, Clone, Copy)]
pub(in crate::physics) struct GridCell {
    pub(in crate::physics) center: Vec2,
    pub(in crate::physics) half: Vec2,
    /// Its layer's layers and mask.
    pub(in crate::physics) layers: u32,
    pub(in crate::physics) mask: u32,
    pub(in crate::physics) identity: u64,
    pub(in crate::physics) cell: TileHit,
    pub(in crate::physics) map_slot: u32,
    pub(in crate::physics) layer_slot: u32,
}

impl CachedMap {
    /// The map as `instance` on `entity` places `data`, its cursor `cursor`,
    /// its collision layers' cells copied and nothing traced.
    fn of(
        entity: Entity,
        instance: &TilemapInstance,
        data: &TilemapData,
        cursor: MapCursor,
    ) -> Self {
        let mut materials: Vec<&str> = Vec::new();
        let classes = (0..data.tileset.len())
            .map(|tile| {
                let props = data.tile_props(tile as TileIndex);
                let material = props.material.as_deref().map_or(0, |name| {
                    let known = materials.iter().position(|&other| other == name);
                    let at = known.unwrap_or_else(|| {
                        materials.push(name);
                        materials.len() - 1
                    });
                    at as u32 + 1
                });
                TileClass {
                    material,
                    one_way: props.one_way,
                    slope: props.slope.map_or(0, crate::assets::Slope::code),
                }
            })
            .collect();
        let chunks = data.width.div_ceil(CHUNK) * data.height.div_ceil(CHUNK);
        Self {
            entity,
            id: instance.id.clone(),
            origin: origin_bits(instance),
            cursor,
            start: 0,
            len: 0,
            at: instance.origin,
            tile: Vec2::new(data.tile_width as f32, data.tile_height as f32),
            width: data.width,
            height: data.height,
            classes,
            layers: data
                .layers
                .iter()
                .enumerate()
                .filter(|(_, layer)| layer.kind == LayerKind::Collision)
                .map(|(index, layer)| {
                    let physics = data.layer_physics(index);
                    CachedLayer {
                        index,
                        layers: Layers {
                            layers: physics.layers,
                            mask: physics.mask,
                        },
                        tiles: layer.tiles.clone(),
                        masks: vec![0; layer.tiles.len()],
                        chunks: vec![Vec::new(); chunks as usize],
                    }
                })
                .collect(),
        }
    }

    /// Whether `instance` on `entity` places this map where it was traced.
    fn placed_by(&self, entity: Entity, instance: &TilemapInstance) -> bool {
        self.entity == entity && self.id == instance.id && self.origin == origin_bits(instance)
    }

    /// Traces chunk `(cx, cy)` of the layer at `slot` again.
    fn trace(&mut self, slot: usize, cx: u32, cy: u32, traced: &mut u64) {
        let chunk = (cy * self.width.div_ceil(CHUNK) + cx) as usize;
        let layer = &mut self.layers[slot];
        let cells = LayerCells {
            width: self.width,
            height: self.height,
            tiles: &layer.tiles,
            classes: &self.classes,
        };
        trace_chunk(&cells, cx, cy, &mut layer.masks, &mut layer.chunks[chunk]);
        *traced += 1;
    }

    /// Traces every chunk of every collision layer.
    fn trace_all(&mut self, traced: &mut u64) {
        let (across, down) = (self.width.div_ceil(CHUNK), self.height.div_ceil(CHUNK));
        for slot in 0..self.layers.len() {
            for cy in 0..down {
                for cx in 0..across {
                    self.trace(slot, cx, cy, traced);
                }
            }
        }
    }

    /// Copies the written `cells` from `data` and re-traces the chunks that
    /// hold each, its four neighbours and each diagonal neighbour that is a
    /// slope, whose mask reads its diagonals, each once.
    fn write(&mut self, data: &TilemapData, cells: &[WrittenCell], traced: &mut u64) {
        let width = self.width as usize;
        let mut dirty: Vec<(usize, u32, u32)> = Vec::new();
        for &(index, row, col) in cells {
            let Some(slot) = self.layers.iter().position(|layer| layer.index == index) else {
                continue;
            };
            let at = (row as usize) * width + col as usize;
            self.layers[slot].tiles[at] = data.layers[index].tiles[at];
            let (c, r) = (i64::from(col), i64::from(row));
            let around = [(0, 0), (0, -1), (1, 0), (0, 1), (-1, 0)];
            let diagonals = [(1, -1), (1, 1), (-1, 1), (-1, -1)];
            for (dc, dr) in around.into_iter().chain(diagonals) {
                let (Ok(col), Ok(row)) = (u32::try_from(c + dc), u32::try_from(r + dr)) else {
                    continue;
                };
                if col >= self.width || row >= self.height {
                    continue;
                }
                if dc != 0 && dr != 0 {
                    let tile = self.layers[slot].tiles[(row as usize) * width + col as usize];
                    let sloped = usize::try_from(tile)
                        .ok()
                        .and_then(|tile| self.classes.get(tile))
                        .is_some_and(|class| class.slope != 0);
                    if !sloped {
                        continue;
                    }
                }
                dirty.push((slot, row / CHUNK, col / CHUNK));
            }
        }
        dirty.sort_unstable();
        dirty.dedup();
        for (slot, cy, cx) in dirty {
            self.trace(slot, cx, cy, traced);
        }
    }

    /// Appends the map's proxies to `arrays`: each solid cell's static box
    /// with the mask its trace gave, its material's restitution and
    /// friction from `coefficients` ([`tile_coefficients`]), its layer's
    /// layers and its sensor identity, in layer, row and column order.
    fn push_proxies(&self, arrays: &mut TileArrays, coefficients: &[PhysicsMaterial]) {
        let half = Vec2::new(self.tile.x * 0.5, self.tile.y * 0.5);
        for layer in &self.layers {
            for row in 0..self.height {
                for col in 0..self.width {
                    let at = (row as usize) * (self.width as usize) + (col as usize);
                    let tile = layer.tiles[at];
                    if tile < 0 {
                        continue;
                    }
                    let center = Vec2::new(
                        self.at.x + (col as f32) * self.tile.x + half.x,
                        self.at.y + (row as f32) * self.tile.y + half.y,
                    );
                    arrays.proxies.push(Proxy {
                        entity: None,
                        key: cell_key(center, self.one_way(tile), self.slope(tile)),
                        center,
                        prev_center: center,
                        velocity: Vec2::ZERO,
                        offset: Vec2::ZERO,
                        shape: Shape::Aabb { half_extents: half },
                        is_dynamic: false,
                        has_velocity: false,
                        inv_mass: 0.0,
                        restitution: coefficients
                            .get(tile as usize)
                            .map_or(0.0, |material| material.restitution),
                        face_mask: layer.masks[at],
                        slope: self.slope(tile),
                        sleeping: false,
                        kinematic: false,
                        kept_awake: false,
                    });
                    arrays.layers.push(layer.layers);
                    arrays
                        .ids
                        .push(tile_identity(self.entity, layer.index, col, row));
                    let material = coefficients.get(tile as usize).copied().unwrap_or_default();
                    arrays.push_coefficients(material.friction, material.rolling);
                }
            }
        }
    }

    /// Appends the map's runs to `runs` in their first cell's tile order,
    /// this map being at `map_slot`.
    fn push_runs(&self, map_slot: usize, runs: &mut Vec<TileRun>) {
        let across = self.width.div_ceil(CHUNK) as usize;
        let mut cursors: Vec<usize> = Vec::new();
        for (slot, layer) in self.layers.iter().enumerate() {
            for (cy, chunk_row) in layer.chunks.chunks(across.max(1)).enumerate() {
                cursors.clear();
                cursors.resize(chunk_row.len(), 0);
                let rows = cy as u32 * CHUNK..((cy as u32 + 1) * CHUNK).min(self.height);
                for row in rows {
                    for (chunk, at) in chunk_row.iter().zip(cursors.iter_mut()) {
                        while let Some(run) = chunk.get(*at).filter(|run| run.row == row) {
                            runs.push(self.run_of(map_slot, slot, run));
                            *at += 1;
                        }
                    }
                }
            }
        }
    }

    /// `run` of the layer at `slot` as the index holds it.
    fn run_of(&self, map_slot: usize, slot: usize, run: &Run) -> TileRun {
        let layer = &self.layers[slot];
        let half = Vec2::new(
            (run.cols as f32 * self.tile.x) * 0.5,
            (run.rows as f32 * self.tile.y) * 0.5,
        );
        TileRun {
            center: Vec2::new(
                self.at.x + (run.col as f32) * self.tile.x + half.x,
                self.at.y + (run.row as f32) * self.tile.y + half.y,
            ),
            half,
            face_mask: run.mask,
            layers: layer.layers.layers,
            mask: layer.layers.mask,
            identity: run_key(self.entity, layer.index, run),
            map_slot: map_slot as u32,
            layer_slot: slot as u32,
            col: run.col,
            row: run.row,
            cols: run.cols,
            rows: run.rows,
        }
    }

    /// The columns and rows of the cells whose boxes may meet `[min, max]`.
    fn span(&self, min: Vec2, max: Vec2) -> Option<(RangeInclusive<u32>, RangeInclusive<u32>)> {
        if self.width == 0 || self.height == 0 {
            return None;
        }
        let low = ((min - self.at) / self.tile).floor();
        let high = ((max - self.at) / self.tile).floor();
        let meets = high.x >= 0.0
            && high.y >= 0.0
            && low.x < self.width as f32
            && low.y < self.height as f32;
        if !meets {
            return None;
        }
        let cols = low.x.max(0.0) as u32..=high.x.min((self.width - 1) as f32) as u32;
        let rows = low.y.max(0.0) as u32..=high.y.min((self.height - 1) as f32) as u32;
        Some((cols, rows))
    }

    /// Whether `tile` is one-way.
    fn one_way(&self, tile: TileIndex) -> bool {
        self.classes
            .get(tile as usize)
            .is_some_and(|class| class.one_way)
    }

    /// `tile`'s slope code, 0 for a full cell.
    fn slope(&self, tile: TileIndex) -> u8 {
        self.classes
            .get(tile as usize)
            .map_or(0, |class| class.slope)
    }

    /// The cell at `(col, row)` of the layer at `slot` when it holds a solid
    /// tile, one-way or not, this map being at `map_slot`.
    fn cell(&self, map_slot: usize, slot: usize, col: u32, row: u32) -> Option<GridCell> {
        let layer = &self.layers[slot];
        let tile = layer.tiles[(row as usize) * (self.width as usize) + (col as usize)];
        if tile < 0 {
            return None;
        }
        let half = Vec2::new(self.tile.x * 0.5, self.tile.y * 0.5);
        Some(GridCell {
            center: Vec2::new(
                self.at.x + (col as f32) * self.tile.x + half.x,
                self.at.y + (row as f32) * self.tile.y + half.y,
            ),
            half,
            layers: layer.layers.layers,
            mask: layer.layers.mask,
            identity: tile_identity(self.entity, layer.index, col, row),
            cell: TileHit {
                map: self.entity,
                layer: layer.index,
                col,
                row,
            },
            map_slot: map_slot as u32,
            layer_slot: slot as u32,
        })
    }

    /// [`Self::cell`] for region queries and sensors, which leave one-way
    /// cells out: they are surfaces, not solids.
    fn solid_cell(&self, map_slot: usize, slot: usize, col: u32, row: u32) -> Option<GridCell> {
        let tile = self.layers[slot].tiles[(row as usize) * (self.width as usize) + (col as usize)];
        if self.one_way(tile) {
            return None;
        }
        self.cell(map_slot, slot, col, row)
    }

    /// Calls `visit` with each solid cell, one-way cells left out, whose box
    /// may meet `[min, max]`, in layer, row and column order, this map being
    /// at `map_slot`.
    fn cells_in(&self, map_slot: usize, min: Vec2, max: Vec2, visit: &mut impl FnMut(&GridCell)) {
        let Some((cols, rows)) = self.span(min, max) else {
            return;
        };
        for slot in 0..self.layers.len() {
            for row in rows.clone() {
                for col in cols.clone() {
                    if let Some(cell) = self.solid_cell(map_slot, slot, col, row) {
                        visit(&cell);
                    }
                }
            }
        }
    }
}

/// A collision cell a refresh found written: its layer's index, its row
/// and its column, in that order so the cells sort in tile order.
type WrittenCell = (usize, u32, u32);

fn origin_bits(instance: &TilemapInstance) -> [u32; 2] {
    [instance.origin.x.to_bits(), instance.origin.y.to_bits()]
}

/// The physics material coefficients of each of `data`'s tileset entries,
/// from its `material` and the World's `materials`: empty when no entry
/// names a material `materials` holds, every tile then taking 0.
fn tile_coefficients(
    data: &TilemapData,
    materials: Option<&PhysicsMaterials>,
) -> Vec<PhysicsMaterial> {
    let Some(materials) = materials else {
        return Vec::new();
    };
    let of = |tile: usize| {
        let props = data.tile_props(tile as TileIndex);
        props
            .material
            .as_deref()
            .and_then(|name| materials.get(name))
    };
    let tiles = data.tileset.len();
    if (0..tiles).all(|tile| of(tile).is_none()) {
        return Vec::new();
    }
    (0..tiles)
        .map(|tile| of(tile).unwrap_or_default())
        .collect()
}

/// Tile proxies with each one's layers, sensor identity and friction, in
/// one order.
#[derive(Debug, Default)]
pub(super) struct TileArrays {
    pub(super) proxies: Vec<Proxy>,
    pub(super) layers: Vec<Layers>,
    pub(super) ids: Vec<u64>,
    /// Each proxy's friction; empty while every one is 0.
    pub(super) frictions: Vec<f32>,
    /// Each proxy's rolling resistance; empty while every one is 0.
    pub(super) rollings: Vec<f32>,
}

impl TileArrays {
    fn clear(&mut self) {
        self.proxies.clear();
        self.layers.clear();
        self.ids.clear();
        self.frictions.clear();
        self.rollings.clear();
    }

    /// Records the friction and rolling resistance of the proxy just
    /// pushed, each list once one of its values is above 0.
    fn push_coefficients(&mut self, friction: f32, rolling: f32) {
        let before = self.proxies.len() - 1;
        for (values, value) in [
            (&mut self.frictions, friction),
            (&mut self.rollings, rolling),
        ] {
            if value != 0.0 || !values.is_empty() {
                values.resize(before, 0.0);
                values.push(value);
            }
        }
    }
}

/// What a refresh does with a map it held.
enum Refresh {
    /// Its proxies, masks and runs stand.
    Keep(usize),
    /// Its masks and runs stand, its materials resolve again and its
    /// proxies are rebuilt: the `PhysicsMaterials` resource changed.
    Restamp(usize),
    /// Its collision cells listed were written: re-trace around them.
    Written(usize, Vec<WrittenCell>),
    /// New, moved, renamed or replaced: trace it whole.
    Whole,
}

impl TileCache {
    /// The tile proxies the gather appends.
    pub(super) fn len(&self) -> usize {
        self.proxies.len()
    }

    /// Bumped by every change to the proxies; 0 before the first.
    pub(super) fn version(&self) -> u64 {
        self.state.version
    }

    /// The outline's runs, in their first cell's tile order.
    pub(super) fn runs(&self) -> &[TileRun] {
        &self.state.runs
    }

    /// Each tile proxy's friction, in gather order; empty while every
    /// tile's is 0.
    pub(super) fn frictions(&self) -> &[f32] {
        &self.state.frictions
    }

    /// Each tile proxy's rolling resistance, in gather order; empty while
    /// every tile's is 0.
    pub(super) fn rollings(&self) -> &[f32] {
        &self.state.rollings
    }

    /// The cell the tile proxy at `ordinal` stands for, its centre `center`:
    /// its map, layer, column and row, one-way cells included.
    pub(super) fn cell_hit(&self, ordinal: usize, center: Vec2) -> Option<TileHit> {
        let identity = *self.ids.get(ordinal)?;
        let (map_slot, map) = self
            .state
            .maps
            .iter()
            .enumerate()
            .find(|(_, map)| (map.start..map.start + map.len).contains(&ordinal))?;
        let (cols, rows) = map.span(center, center)?;
        for slot in 0..map.layers.len() {
            for row in rows.clone() {
                for col in cols.clone() {
                    if let Some(cell) = map.cell(map_slot, slot, col, row)
                        && cell.identity == identity
                    {
                        return Some(cell.cell);
                    }
                }
            }
        }
        None
    }

    /// Chunks traced since the cache was made.
    pub(super) fn traced(&self) -> u64 {
        self.state.traced
    }

    /// The maps whose every cell the last refresh changed: replaced, added,
    /// moved, resized or gone.
    pub(super) fn replaced(&self) -> &[Entity] {
        &self.state.replaced
    }

    /// The sensor identities of the collision cells the last refresh found
    /// written in the other maps, sorted.
    pub(super) fn written_ids(&self) -> &[u64] {
        &self.state.written_ids
    }

    /// Maps traced, whole or in part, since the cache was made.
    #[cfg(test)]
    pub(super) fn scans(&self) -> usize {
        self.state.scans
    }

    /// Calls `visit` with each solid cell of the last refresh, one-way cells
    /// left out, whose box may meet `[min, max]`, in tile order: by map,
    /// layer, row and column.
    pub(in crate::physics) fn cells_in(
        &self,
        min: Vec2,
        max: Vec2,
        mut visit: impl FnMut(&GridCell),
    ) {
        for (slot, map) in self.state.maps.iter().enumerate() {
            map.cells_in(slot, min, max, &mut visit);
        }
    }

    /// Calls `visit` with each solid cell the last refresh changed, one-way
    /// cells left out, whose box may meet `[min, max]`, in tile order: every
    /// cell of a replaced map, the written ones of the rest.
    pub(in crate::physics) fn changed_cells_in(
        &self,
        min: Vec2,
        max: Vec2,
        mut visit: impl FnMut(&GridCell),
    ) {
        for (map_slot, map) in self.state.maps.iter().enumerate() {
            if self.state.replaced.contains(&map.entity) {
                map.cells_in(map_slot, min, max, &mut visit);
                continue;
            }
            let Some((_, cells)) = self
                .state
                .written
                .iter()
                .find(|(entity, _)| *entity == map.entity)
            else {
                continue;
            };
            let Some((cols, rows)) = map.span(min, max) else {
                continue;
            };
            for &(index, row, col) in cells {
                if !(cols.contains(&col) && rows.contains(&row)) {
                    continue;
                }
                let slot = map.layers.iter().position(|layer| layer.index == index);
                if let Some(cell) = slot.and_then(|slot| map.solid_cell(map_slot, slot, col, row)) {
                    visit(&cell);
                }
            }
        }
    }

    /// Calls `visit` with each cell of `run` whose box may meet `[min, max]`,
    /// in row and column order.
    pub(in crate::physics) fn run_cells_in(
        &self,
        run: &TileRun,
        min: Vec2,
        max: Vec2,
        mut visit: impl FnMut(&GridCell),
    ) {
        let map = &self.state.maps[run.map_slot as usize];
        let Some((cols, rows)) = map.span(min, max) else {
            return;
        };
        let cols = (*cols.start()).max(run.col)..=(*cols.end()).min(run.col + run.cols - 1);
        let rows = (*rows.start()).max(run.row)..=(*rows.end()).min(run.row + run.rows - 1);
        for row in rows {
            for col in cols.clone() {
                let slot = run.layer_slot as usize;
                if let Some(cell) = map.cell(run.map_slot as usize, slot, col, row) {
                    visit(&cell);
                }
            }
        }
    }

    /// Brings the cache up to `world`'s maps: an unchanged map keeps its
    /// proxies, a written one re-traces the chunks around its written cells
    /// and rebuilds its proxies, a new, moved or replaced one is traced
    /// whole, and one that went drops; what changed is noted for the sensor
    /// pass.
    fn refresh(&mut self, world: &World) {
        let state = &mut *self.state;
        state.replaced.clear();
        state.written.clear();
        state.written_ids.clear();
        let registry = world.get_resource::<TilemapRegistry>();
        let materials = world.get_resource::<PhysicsMaterials>();
        let materials_version = materials.map(PhysicsMaterials::version);
        let restamp = materials_version != state.materials_version;
        state.materials_version = materials_version;
        if !restamp && Self::matches(&state.maps, world, registry) {
            return;
        }
        let mut kept = std::mem::take(&mut state.maps);
        let order: Vec<Entity> = kept.iter().map(|map| map.entity).collect();
        let old = TileArrays {
            proxies: std::mem::take(&mut self.proxies),
            layers: std::mem::take(&mut self.layers),
            ids: std::mem::take(&mut self.ids),
            frictions: std::mem::take(&mut state.frictions),
            rollings: std::mem::take(&mut state.rollings),
        };
        let mut arrays = std::mem::take(&mut state.spare);
        arrays.clear();
        let mut changed = false;
        for (entity, instance) in world.query::<(Entity, &TilemapInstance)>() {
            let Some((data, cursor, registry)) = registry.and_then(|registry| {
                Some((
                    registry.get(&instance.id)?,
                    registry.cursor(&instance.id)?,
                    registry,
                ))
            }) else {
                continue;
            };
            let at = kept.iter().position(|map| map.placed_by(entity, instance));
            let refresh = match at {
                None => Refresh::Whole,
                Some(at) => match registry.changes_since(&kept[at].id, &kept[at].cursor) {
                    Some(TileChanges::Unchanged) if restamp => Refresh::Restamp(at),
                    Some(TileChanges::Unchanged) => Refresh::Keep(at),
                    Some(TileChanges::Cells(cells)) => {
                        let mut cells: Vec<WrittenCell> = cells
                            .filter(|cell| {
                                data.layers
                                    .get(cell.layer)
                                    .is_some_and(|layer| layer.kind == LayerKind::Collision)
                            })
                            .map(|cell| (cell.layer, cell.row, cell.col))
                            .collect();
                        cells.sort_unstable();
                        cells.dedup();
                        if cells.is_empty() && restamp {
                            Refresh::Restamp(at)
                        } else if cells.is_empty() {
                            Refresh::Keep(at)
                        } else {
                            Refresh::Written(at, cells)
                        }
                    }
                    _ => Refresh::Whole,
                },
            };
            let start = arrays.proxies.len();
            let mut map = match refresh {
                Refresh::Keep(at) => {
                    let mut map = kept.swap_remove(at);
                    let range = map.start..map.start + map.len;
                    arrays
                        .proxies
                        .extend_from_slice(&old.proxies[range.clone()]);
                    arrays.layers.extend_from_slice(&old.layers[range.clone()]);
                    arrays.ids.extend_from_slice(&old.ids[range.clone()]);
                    for (values, old) in [
                        (&mut arrays.frictions, &old.frictions),
                        (&mut arrays.rollings, &old.rollings),
                    ] {
                        if !old.is_empty() {
                            values.resize(start, 0.0);
                            values.extend_from_slice(&old[range.clone()]);
                        }
                    }
                    map.start = start;
                    map.cursor = cursor;
                    state.maps.push(map);
                    continue;
                }
                Refresh::Restamp(at) => kept.swap_remove(at),
                Refresh::Written(at, cells) => {
                    let mut map = kept.swap_remove(at);
                    map.write(data, &cells, &mut state.traced);
                    state.written_ids.extend(
                        cells
                            .iter()
                            .map(|&(layer, row, col)| tile_identity(entity, layer, col, row)),
                    );
                    state.written.push((entity, cells));
                    map
                }
                Refresh::Whole => {
                    // A map traced again leaves its old entry, which went
                    // with it.
                    if let Some(at) = at {
                        kept.swap_remove(at);
                    }
                    let mut map = CachedMap::of(entity, instance, data, cursor);
                    map.trace_all(&mut state.traced);
                    state.replaced.push(entity);
                    map
                }
            };
            changed = true;
            #[cfg(test)]
            {
                state.scans += 1;
            }
            map.cursor = cursor;
            map.start = start;
            map.push_proxies(&mut arrays, &tile_coefficients(data, materials));
            map.len = arrays.proxies.len() - start;
            state.maps.push(map);
        }
        for values in [&mut arrays.frictions, &mut arrays.rollings] {
            if !values.is_empty() {
                values.resize(arrays.proxies.len(), 0.0);
            }
        }
        // What is left went, or now places another map elsewhere.
        for gone in &kept {
            if !state.replaced.contains(&gone.entity) {
                state.replaced.push(gone.entity);
            }
        }
        let same = !changed && kept.is_empty() && state.maps.iter().map(|map| map.entity).eq(order);
        state.written_ids.sort_unstable();
        state.layered = arrays
            .layers
            .iter()
            .any(|&layers| layers != Layers::DEFAULT);
        self.proxies = arrays.proxies;
        self.layers = arrays.layers;
        self.ids = arrays.ids;
        state.frictions = arrays.frictions;
        state.rollings = arrays.rollings;
        state.spare = old;
        if !same {
            state.runs.clear();
            for (slot, map) in state.maps.iter().enumerate() {
                map.push_runs(slot, &mut state.runs);
            }
            state.version += 1;
        }
    }

    /// Whether the maps with registered data, in query order, are the ones
    /// in `maps`, each placed as it was and unchanged.
    fn matches(maps: &[CachedMap], world: &World, registry: Option<&TilemapRegistry>) -> bool {
        let Some(registry) = registry else {
            return maps.is_empty();
        };
        let mut cached = maps.iter();
        for (entity, instance) in world.query::<(Entity, &TilemapInstance)>() {
            if registry.get(&instance.id).is_none() {
                continue;
            }
            let unchanged = cached.next().is_some_and(|map| {
                map.placed_by(entity, instance)
                    && matches!(
                        registry.changes_since(&map.id, &map.cursor),
                        Some(TileChanges::Unchanged)
                    )
            });
            if !unchanged {
                return false;
            }
        }
        cached.next().is_none()
    }
}

/// A fresh trace of `world`'s maps, as a cache made this frame holds it:
/// the oracle of the kept runs.
#[cfg(test)]
pub(super) fn trace_tilemaps(world: &World) -> TileCache {
    let mut fresh = TileCache::default();
    fresh.refresh(world);
    fresh
}

/// Scan every map's collision layers cell by cell, as the step did before
/// the outline: the test oracle of the kept proxies.
#[cfg(test)]
pub(super) fn gather_tilemap_proxies(world: &World, arrays: &mut TileArrays) {
    let Some(registry) = world.get_resource::<TilemapRegistry>() else {
        return;
    };
    for (map, instance) in world.query::<(Entity, &TilemapInstance)>() {
        if let Some(data) = registry.get(&instance.id) {
            scan_map(map, instance, data, arrays);
        }
    }
}

/// Scan one map's collision layers into `arrays` as the step did before the
/// outline: each solid cell's static proxy and its face mask, its layer's
/// layers and its sensor identity, in layer, row and column order.
#[cfg(test)]
fn scan_map(map: Entity, instance: &TilemapInstance, data: &TilemapData, arrays: &mut TileArrays) {
    let tw = data.tile_width as f32;
    let th = data.tile_height as f32;
    let half = Vec2::new(tw * 0.5, th * 0.5);

    for (layer_index, layer) in data.layers.iter().enumerate() {
        if layer.kind != LayerKind::Collision {
            continue;
        }
        let physics = data.layer_physics(layer_index);
        let layer_layers = Layers {
            layers: physics.layers,
            mask: physics.mask,
        };
        let w = data.width as i32;
        let h = data.height as i32;
        let is_solid = |c: i32, r: i32| -> bool {
            if c < 0 || r < 0 || c >= w || r >= h {
                return false;
            }
            let idx = (r as usize) * (data.width as usize) + (c as usize);
            layer.tiles[idx] >= 0
        };
        for row in 0..data.height {
            for col in 0..data.width {
                let idx = (row as usize) * (data.width as usize) + (col as usize);
                let tile = layer.tiles[idx];
                if tile < 0 {
                    continue;
                }
                let c = col as i32;
                let r = row as i32;
                // Solid neighbors mask internal faces; map edges remain exposed.
                let mut face_mask: u8 = 0;
                if !is_solid(c, r - 1) {
                    face_mask |= FACE_TOP;
                }
                if !is_solid(c, r + 1) {
                    face_mask |= FACE_BOTTOM;
                }
                if !is_solid(c - 1, r) {
                    face_mask |= FACE_LEFT;
                }
                if !is_solid(c + 1, r) {
                    face_mask |= FACE_RIGHT;
                }
                let center = Vec2::new(
                    instance.origin.x + (col as f32) * tw + half.x,
                    instance.origin.y + (row as f32) * th + half.y,
                );
                arrays.proxies.push(Proxy {
                    entity: None,
                    key: tile_key(center),
                    center,
                    prev_center: center,
                    velocity: Vec2::ZERO,
                    offset: Vec2::ZERO,
                    shape: Shape::Aabb { half_extents: half },
                    is_dynamic: false,
                    has_velocity: false,
                    inv_mass: 0.0,
                    restitution: 0.0,
                    face_mask,
                    slope: 0,
                    sleeping: false,
                    kinematic: false,
                    kept_awake: false,
                });
                arrays.layers.push(layer_layers);
                arrays.ids.push(tile_identity(map, layer_index, col, row));
            }
        }
    }
}
