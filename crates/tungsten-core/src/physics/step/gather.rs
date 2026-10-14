//! Once-per-frame gather of entity and tilemap collider proxies in
//! deterministic `World` order (D-066), and their warm-start keys; sensors
//! are gathered beside the proxies, never as one.

use super::{Proxy, SensorBody};
use crate::assets::{LayerKind, TilemapInstance, TilemapRegistry};
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
/// `prev_center == center` at gather time. A sensor row joins `sensors`
/// instead of the proxies; in a frame with sensors, `tile_ids` gets each
/// tile proxy's identity, in proxy order, and stays empty otherwise.
/// Returns how many kinematic proxies it staged, which sets the frame's
/// substep count.
pub(super) fn gather_proxies(
    world: &World,
    proxies: &mut Vec<Proxy>,
    sensors: &mut Vec<SensorBody>,
    tile_ids: &mut Vec<u64>,
) -> usize {
    proxies.clear();
    sensors.clear();
    tile_ids.clear();
    let mut kinematics = 0;

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
        if body.is_some_and(|b| b.kind == BodyKind::Sensor) {
            sensors.push(SensorBody {
                entity,
                key: entity_key(entity),
                center: position.0 + collider.offset,
                shape: collider.shape,
                velocity,
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
    }

    let tile_ids = (!sensors.is_empty()).then_some(tile_ids);
    gather_tilemap_proxies(world, proxies, tile_ids);
    kinematics
}

/// Emit transient static tile proxies for collision layers, and their
/// identities when `tile_ids` is given.
fn gather_tilemap_proxies(
    world: &World,
    proxies: &mut Vec<Proxy>,
    mut tile_ids: Option<&mut Vec<u64>>,
) {
    let Some(registry) = world.get_resource::<TilemapRegistry>() else {
        return;
    };

    for (map, instance) in world.query::<(Entity, &TilemapInstance)>() {
        let Some(data) = registry.get(&instance.id) else {
            continue;
        };
        let tw = data.tile_width as f32;
        let th = data.tile_height as f32;
        let half = Vec2::new(tw * 0.5, th * 0.5);

        for (layer_index, layer) in data.layers.iter().enumerate() {
            if layer.kind != LayerKind::Collision {
                continue;
            }
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
                    if let Some(ids) = tile_ids.as_deref_mut() {
                        ids.push(tile_identity(map, layer_index, col, row));
                    }
                }
            }
        }
    }
}
