//! Once-per-frame gather of entity and tilemap collider proxies in
//! deterministic `World` order (D-066), their warm-start keys and their
//! layers; sensors are gathered beside the proxies, never as one. Tile
//! proxies come from a cache of the collision layers that is checked every
//! frame and rescanned only when a map changed; each is in its collision
//! layer's physics layers.

use super::{Layers, Proxy, ProxyLayers, SensorBody};
use crate::assets::{
    LayerKind, LayerPhysics, TileIndex, TilemapData, TilemapInstance, TilemapRegistry,
};
use crate::ecs::{Entity, World};
use crate::physics::collision::{FACE_ALL, FACE_BOTTOM, FACE_LEFT, FACE_RIGHT, FACE_TOP};
use crate::physics::components::{BodyKind, Collider, Position, RigidBody, Shape, Velocity};
use glam::Vec2;

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
/// physics layers, as the last scan built them in gather order, with each
/// one's sensor identity and the maps they came from. The gather checks the
/// maps every frame, the tiles and physics of each collision layer compared
/// whole, and scans again only when one differs, so the proxies, their
/// order, keys, face masks, layers and identities are what a scan of this
/// frame would give.
#[derive(Debug, Default)]
pub(super) struct TileCache {
    /// Bumped by every scan; 0 before the first.
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
    #[cfg(test)]
    pub(super) scans: usize,
}

/// One map as the last scan read it: what a later frame compares.
#[derive(Debug)]
pub(super) struct CachedMap {
    entity: Entity,
    origin: [u32; 2],
    tile: [u32; 2],
    size: [u32; 2],
    /// Each collision layer's index, physics and tiles.
    layers: Vec<(usize, LayerPhysics, Vec<TileIndex>)>,
}

impl CachedMap {
    /// Whether `data`, placed by `instance` on `entity`, is this map.
    fn is(&self, entity: Entity, instance: &TilemapInstance, data: &TilemapData) -> bool {
        let mut collision = data
            .layers
            .iter()
            .enumerate()
            .filter(|(_, layer)| layer.kind == LayerKind::Collision);
        self.entity == entity
            && self.origin == [instance.origin.x.to_bits(), instance.origin.y.to_bits()]
            && self.tile == [data.tile_width, data.tile_height]
            && self.size == [data.width, data.height]
            && self.layers.iter().all(|(index, physics, tiles)| {
                collision.next().is_some_and(|(at, layer)| {
                    at == *index && data.layer_physics(at) == *physics && layer.tiles == *tiles
                })
            })
            && collision.next().is_none()
    }
}

impl TileCache {
    /// The tile proxies the gather appends.
    pub(super) fn len(&self) -> usize {
        self.proxies.len()
    }

    /// Brings the cache up to `world`'s maps: unchanged maps keep it, any
    /// difference scans them all again.
    fn refresh(&mut self, world: &World) {
        if self.matches(world) {
            return;
        }
        self.scan(world);
    }

    /// Whether the maps with registered data, in query order, are the ones
    /// the last scan read.
    fn matches(&self, world: &World) -> bool {
        let Some(registry) = world.get_resource::<TilemapRegistry>() else {
            return self.maps.is_empty();
        };
        let mut cached = self.maps.iter();
        for (map, instance) in world.query::<(Entity, &TilemapInstance)>() {
            let Some(data) = registry.get(&instance.id) else {
                continue;
            };
            if !cached.next().is_some_and(|old| old.is(map, instance, data)) {
                return false;
            }
        }
        cached.next().is_none()
    }

    /// Scans every collision layer of every map again.
    fn scan(&mut self, world: &World) {
        self.proxies.clear();
        self.layers.clear();
        self.ids.clear();
        self.maps.clear();
        gather_tilemap_proxies(
            world,
            &mut self.proxies,
            &mut self.layers,
            &mut self.ids,
            &mut self.maps,
        );
        self.layered = self.layers.iter().any(|&layers| layers != Layers::DEFAULT);
        self.version += 1;
        #[cfg(test)]
        {
            self.scans += 1;
        }
    }
}

/// Scan the static tile proxies of collision layers, their layers, their
/// identities and the maps they came from.
pub(super) fn gather_tilemap_proxies(
    world: &World,
    proxies: &mut Vec<Proxy>,
    tile_layers: &mut Vec<Layers>,
    tile_ids: &mut Vec<u64>,
    maps: &mut Vec<CachedMap>,
) {
    let Some(registry) = world.get_resource::<TilemapRegistry>() else {
        return;
    };

    for (map, instance) in world.query::<(Entity, &TilemapInstance)>() {
        let Some(data) = registry.get(&instance.id) else {
            continue;
        };
        maps.push(CachedMap {
            entity: map,
            origin: [instance.origin.x.to_bits(), instance.origin.y.to_bits()],
            tile: [data.tile_width, data.tile_height],
            size: [data.width, data.height],
            layers: data
                .layers
                .iter()
                .enumerate()
                .filter(|(_, layer)| layer.kind == LayerKind::Collision)
                .map(|(index, layer)| (index, data.layer_physics(index), layer.tiles.clone()))
                .collect(),
        });
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
                    proxies.push(Proxy {
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
                    tile_layers.push(layer_layers);
                    tile_ids.push(tile_identity(map, layer_index, col, row));
                }
            }
        }
    }
}
