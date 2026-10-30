//! Once-per-frame gather of entity and tilemap collider proxies in
//! deterministic `World` order (D-066), their warm-start keys and their
//! layers; sensors are gathered beside the proxies, never as one. Tile
//! proxies come from a cache of the collision layers that asks the registry
//! every frame whether a map changed (its cursor) and rescans that map
//! alone; each is in its collision layer's physics layers.

use super::{Layers, Proxy, ProxyLayers, SensorBody};
use crate::assets::{
    LayerKind, MapCursor, TileChanges, TileIndex, TilemapData, TilemapInstance, TilemapRegistry,
};
use crate::ecs::{Entity, World};
use crate::physics::collision::{FACE_ALL, FACE_BOTTOM, FACE_LEFT, FACE_RIGHT, FACE_TOP};
use crate::physics::components::{BodyKind, Collider, Position, RigidBody, Shape, Velocity};
use crate::physics::query::TileHit;
use glam::Vec2;
use std::ops::RangeInclusive;

/// Warm-start identity for entity proxies; tag bit 63 separates the entity
/// keyspace from tile hashes.
pub(super) fn entity_key(entity: Entity) -> u64 {
    (1 << 63) | (u64::from(entity.generation & 0x7FFF_FFFF) << 32) | u64::from(entity.index)
}

/// Warm-start identity for tile proxies, hashed from the tile center; stable
/// while the tilemap does not move (a moved map only loses warm starts).
fn tile_key(center: Vec2) -> u64 {
    let h = u64::from(center.x.to_bits()).wrapping_mul(0x8DA6_B343_D816_3841)
        ^ u64::from(center.y.to_bits()).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let h = h ^ (h >> 31);
    h & !(1 << 63)
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

/// Gather entity + tilemap proxies in deterministic `World` order, once per
/// frame (D-066); the gather runs before any integration, so
/// `prev_center == center` at gather time. Each proxy's layers go to
/// `layers`, in proxy order, which notes whether any proxy or sensor is off
/// the defaults. A sensor row joins `sensors` instead of the proxies; in a
/// frame with sensors, `tile_ids` gets each tile proxy's identity, in proxy
/// order, and stays empty otherwise. The tile proxies are `tiles`', brought
/// up to the World's maps first. Returns how many kinematic proxies it
/// staged, which sets the frame's substep count.
pub(super) fn gather_proxies(
    world: &World,
    proxies: &mut Vec<Proxy>,
    layers: &mut ProxyLayers,
    sensors: &mut Vec<SensorBody>,
    tile_ids: &mut Vec<u64>,
    tiles: &mut TileCache,
) -> usize {
    proxies.clear();
    layers.of.clear();
    sensors.clear();
    tile_ids.clear();
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
            sleeping: false,
            kinematic,
            kept_awake: false,
        });
        layers.of.push(body_layers);
    }

    tiles.refresh(world);
    proxies.extend_from_slice(&tiles.proxies);
    layers.of.extend_from_slice(&tiles.layers);
    layers.layered = layered || tiles.layered;
    if !sensors.is_empty() {
        tile_ids.extend_from_slice(&tiles.ids);
    }
    kinematics
}

/// The static tile proxies of every collision layer, each in its layer's
/// physics layers, in gather order: each map's, in query order, as its last
/// scan built them, with each one's sensor identity and cell; and each
/// collision layer's cells as the last refresh read them, which region
/// queries and sensors read. A map is scanned again only when its registry
/// cursor reads a change to a collision layer, its instance moved or names
/// another map, or it appeared; one that went drops out. So the proxies,
/// their order, keys, face masks, layers, identities and cells are what a
/// scan of this frame would give.
#[derive(Debug, Default)]
pub(in crate::physics) struct TileCache {
    /// Bumped by every change to the proxies; 0 before the first.
    pub(super) version: u64,
    maps: Vec<CachedMap>,
    /// The tile proxies, in gather order.
    pub(super) proxies: Vec<Proxy>,
    /// Each tile proxy's layers and mask, in the same order.
    pub(super) layers: Vec<Layers>,
    /// Some tile proxy is off [`Layers::DEFAULT`].
    layered: bool,
    /// Each tile proxy's identity in sensor overlap keys, in the same order.
    pub(super) ids: Vec<u64>,
    /// Each tile proxy's map, layer and cell, in the same order.
    pub(super) cells: Vec<TileHit>,
    /// The maps whose every cell the last refresh changed: replaced,
    /// added, moved, resized or gone.
    pub(super) replaced: Vec<Entity>,
    /// The collision cells the last refresh found written in the other
    /// maps, by map in cache order, each map's sorted by layer, row and
    /// column.
    written: Vec<(Entity, Vec<WrittenCell>)>,
    /// Those cells' sensor identities, sorted.
    pub(super) written_ids: Vec<u64>,
    /// Maps scanned since the cache was made.
    #[cfg(test)]
    pub(super) scans: usize,
}

/// One map as its last scan read it: where its proxies sit, and its
/// collision layers' cells where its instance placed them.
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
    /// Its collision layers, in layer order.
    layers: Vec<CachedLayer>,
}

/// A collision layer's cells as the cache's last refresh read them.
#[derive(Debug)]
struct CachedLayer {
    /// Its index in the map's layers.
    index: usize,
    layers: Layers,
    tiles: Vec<TileIndex>,
}

/// A solid cell of a collision layer, as region queries and sensors test
/// it: a box exactly as its proxy's was, with its layers, sensor identity
/// and cell.
#[derive(Debug, Clone, Copy)]
pub(in crate::physics) struct GridCell {
    pub(in crate::physics) center: Vec2,
    pub(in crate::physics) half: Vec2,
    /// Its layer's layers and mask.
    pub(in crate::physics) layers: u32,
    pub(in crate::physics) mask: u32,
    pub(in crate::physics) identity: u64,
    pub(in crate::physics) cell: TileHit,
}

impl CachedMap {
    /// The map as `instance` on `entity` places `data`, its cursor `cursor`
    /// and its proxies from `start`, `len` of them.
    fn of(
        entity: Entity,
        instance: &TilemapInstance,
        data: &TilemapData,
        cursor: MapCursor,
        start: usize,
        len: usize,
    ) -> Self {
        Self {
            entity,
            id: instance.id.clone(),
            origin: origin_bits(instance),
            cursor,
            start,
            len,
            at: instance.origin,
            tile: Vec2::new(data.tile_width as f32, data.tile_height as f32),
            width: data.width,
            height: data.height,
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
                    }
                })
                .collect(),
        }
    }

    /// Whether `instance` on `entity` places this map where it was scanned.
    fn placed_by(&self, entity: Entity, instance: &TilemapInstance) -> bool {
        self.entity == entity && self.id == instance.id && self.origin == origin_bits(instance)
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

    /// The cell at `(col, row)` of `layer` when it is solid.
    fn cell(&self, layer: &CachedLayer, col: u32, row: u32) -> Option<GridCell> {
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
        })
    }

    /// Calls `visit` with each solid cell whose box may meet `[min, max]`,
    /// in layer, row and column order.
    fn cells_in(&self, min: Vec2, max: Vec2, visit: &mut impl FnMut(&GridCell)) {
        let Some((cols, rows)) = self.span(min, max) else {
            return;
        };
        for layer in &self.layers {
            for row in rows.clone() {
                for col in cols.clone() {
                    if let Some(cell) = self.cell(layer, col, row) {
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

/// Tile proxies with each one's layers, sensor identity and cell, in one
/// order.
#[derive(Debug, Default)]
pub(super) struct TileArrays {
    pub(super) proxies: Vec<Proxy>,
    pub(super) layers: Vec<Layers>,
    pub(super) ids: Vec<u64>,
    pub(super) cells: Vec<TileHit>,
}

/// What a refresh does with a map it held.
enum Refresh {
    /// Its proxies and cells stand.
    Keep(usize),
    /// Its collision cells listed were written: scan it again.
    Written(Vec<WrittenCell>),
    /// New, moved, renamed or replaced: scan it whole.
    Whole,
}

impl TileCache {
    /// The tile proxies the gather appends.
    pub(super) fn len(&self) -> usize {
        self.proxies.len()
    }

    /// Calls `visit` with each solid cell of the last refresh whose box may
    /// meet `[min, max]`, in tile order: by map, layer, row and column.
    pub(in crate::physics) fn cells_in(
        &self,
        min: Vec2,
        max: Vec2,
        mut visit: impl FnMut(&GridCell),
    ) {
        for map in &self.maps {
            map.cells_in(min, max, &mut visit);
        }
    }

    /// Calls `visit` with each solid cell the last refresh changed whose box
    /// may meet `[min, max]`, in tile order: every cell of a replaced map,
    /// the written ones of the rest.
    pub(in crate::physics) fn changed_cells_in(
        &self,
        min: Vec2,
        max: Vec2,
        mut visit: impl FnMut(&GridCell),
    ) {
        for map in &self.maps {
            if self.replaced.contains(&map.entity) {
                map.cells_in(min, max, &mut visit);
                continue;
            }
            let Some((_, cells)) = self
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
                let layer = map.layers.iter().find(|layer| layer.index == index);
                if let Some(cell) = layer.and_then(|layer| map.cell(layer, col, row)) {
                    visit(&cell);
                }
            }
        }
    }

    /// Brings the cache up to `world`'s maps: an unchanged map keeps its
    /// proxies, a changed or new one is scanned, and one that went drops;
    /// what changed is noted for the sensor pass.
    fn refresh(&mut self, world: &World) {
        self.replaced.clear();
        self.written.clear();
        self.written_ids.clear();
        let registry = world.get_resource::<TilemapRegistry>();
        if self.matches(world, registry) {
            return;
        }
        let mut kept = std::mem::take(&mut self.maps);
        let order: Vec<Entity> = kept.iter().map(|map| map.entity).collect();
        let old = TileArrays {
            proxies: std::mem::take(&mut self.proxies),
            layers: std::mem::take(&mut self.layers),
            ids: std::mem::take(&mut self.ids),
            cells: std::mem::take(&mut self.cells),
        };
        let mut arrays = TileArrays::default();
        let mut scanned = false;
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
                        if cells.is_empty() {
                            Refresh::Keep(at)
                        } else {
                            Refresh::Written(cells)
                        }
                    }
                    _ => Refresh::Whole,
                },
            };
            let start = arrays.proxies.len();
            // A map scanned again leaves its old entry, which went with it.
            if let (Some(at), Refresh::Written(_) | Refresh::Whole) = (at, &refresh) {
                kept.swap_remove(at);
            }
            match refresh {
                Refresh::Keep(at) => {
                    let map = &mut kept[at];
                    let range = map.start..map.start + map.len;
                    arrays
                        .proxies
                        .extend_from_slice(&old.proxies[range.clone()]);
                    arrays.layers.extend_from_slice(&old.layers[range.clone()]);
                    arrays.ids.extend_from_slice(&old.ids[range.clone()]);
                    arrays.cells.extend_from_slice(&old.cells[range]);
                    map.start = start;
                    map.cursor = cursor;
                    let map = kept.swap_remove(at);
                    self.maps.push(map);
                    continue;
                }
                Refresh::Written(cells) => {
                    self.written_ids.extend(
                        cells
                            .iter()
                            .map(|&(layer, row, col)| tile_identity(entity, layer, col, row)),
                    );
                    self.written.push((entity, cells));
                }
                Refresh::Whole => self.replaced.push(entity),
            }
            scanned = true;
            scan_map(entity, instance, data, &mut arrays);
            #[cfg(test)]
            {
                self.scans += 1;
            }
            self.maps.push(CachedMap::of(
                entity,
                instance,
                data,
                cursor,
                start,
                arrays.proxies.len() - start,
            ));
        }
        // What is left went, or now places another map elsewhere.
        for gone in &kept {
            if !self.replaced.contains(&gone.entity) {
                self.replaced.push(gone.entity);
            }
        }
        let same = !scanned && kept.is_empty() && self.maps.iter().map(|map| map.entity).eq(order);
        self.written_ids.sort_unstable();
        self.layered = arrays
            .layers
            .iter()
            .any(|&layers| layers != Layers::DEFAULT);
        self.proxies = arrays.proxies;
        self.layers = arrays.layers;
        self.ids = arrays.ids;
        self.cells = arrays.cells;
        if !same {
            self.version += 1;
        }
    }

    /// Whether the maps with registered data, in query order, are the ones
    /// the cache holds, each placed as it was and unchanged.
    fn matches(&self, world: &World, registry: Option<&TilemapRegistry>) -> bool {
        let Some(registry) = registry else {
            return self.maps.is_empty();
        };
        let mut cached = self.maps.iter();
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

/// Scan every map's collision layers, as a fresh cache would: the test
/// oracle of the kept proxies.
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

/// Scan one map's collision layers into `arrays`: each solid cell's static
/// proxy, its layer's layers, its sensor identity and its cell, in layer,
/// row and column order.
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
                    sleeping: false,
                    kinematic: false,
                    kept_awake: false,
                });
                arrays.layers.push(layer_layers);
                arrays.ids.push(tile_identity(map, layer_index, col, row));
                arrays.cells.push(TileHit {
                    map,
                    layer: layer_index,
                    col,
                    row,
                });
            }
        }
    }
}
