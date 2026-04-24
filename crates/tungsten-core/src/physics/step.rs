//! Physics step: substep -> integrate -> rebuild broadphase -> resolve -> events.
//!
//! Broadphase is rebuilt from scratch each substep; no persistent proxy state.

use super::broadphase::{ProxyId, SpatialGrid};
use super::collision::{
    aabb_vs_aabb_masked, aabb_vs_circle_masked, circle_vs_circle, sweep_aabb_vs_aabb, Aabb,
    Contact, FACE_ALL, FACE_BOTTOM, FACE_LEFT, FACE_RIGHT, FACE_TOP,
};
use super::components::{BodyKind, Collider, Position, RigidBody, Shape, Velocity};
use super::events::CollisionEvent;
use super::PhysicsConfig;
use crate::assets::{LayerKind, TilemapInstance, TilemapRegistry};
use crate::ecs::{Entity, EventQueue, World};
use crate::time::DeltaTime;
use glam::Vec2;

/// Per-substep collider proxy; tile proxies use `entity = None`.
#[derive(Debug, Clone, Copy)]
struct Proxy {
    entity: Option<Entity>,
    center: Vec2,
    /// Pre-integration center for speculative sweep.
    prev_center: Vec2,
    velocity: Vec2,
    offset: Vec2,
    shape: Shape,
    is_dynamic: bool,
    inv_mass: f32,
    restitution: f32,
    /// Tile exposed-face mask; internal faces suppress seam contacts.
    face_mask: u8,
}

impl Proxy {
    fn world_aabb(&self) -> Aabb {
        Aabb::new(self.center, self.half_extents())
    }

    fn half_extents(&self) -> Vec2 {
        match self.shape {
            Shape::Aabb { half_extents } => half_extents,
            Shape::Circle { radius } => Vec2::splat(radius),
        }
    }

    fn min_half_extent(&self) -> f32 {
        match self.shape {
            Shape::Aabb { half_extents } => half_extents.x.min(half_extents.y),
            Shape::Circle { radius } => radius,
        }
    }

    /// Union of pre/post AABBs for swept broadphase queries.
    fn swept_aabb(&self) -> Aabb {
        let cur = self.world_aabb();
        if self.prev_center == self.center {
            return cur;
        }
        let prev = Aabb::new(self.prev_center, self.half_extents());
        cur.union(&prev)
    }
}

/// Physics scratch buffers reused across substeps/frames.
#[derive(Debug, Default)]
pub struct PhysicsBuffers {
    proxies: Vec<Proxy>,
    pairs: Vec<(u32, u32)>,
    events: Vec<CollisionEvent>,
    candidates: Vec<ProxyId>,
    grid: SpatialGrid,
    collider_entities: Vec<Entity>,
    dynamic_entities: Vec<Entity>,
}

/// Run one physics tick with velocity-derived substeps.
pub fn physics_step(world: &mut World) {
    let dt = world
        .get_resource::<DeltaTime>()
        .map_or(0.0, super::super::time::DeltaTime::seconds);
    if dt <= 0.0 {
        return;
    }

    let config = world
        .get_resource::<PhysicsConfig>()
        .copied()
        .unwrap_or_default();

    let substeps = compute_substeps(world, dt, &config);
    let sub_dt = dt / substeps as f32;

    // Remove/reinsert buffers to avoid resource borrow + entity borrow overlap.
    let mut buffers = world
        .remove_resource::<PhysicsBuffers>()
        .unwrap_or_default();

    for _ in 0..substeps {
        apply_gravity_and_integrate(world, sub_dt, config.gravity, &mut buffers);
        resolve_collisions(world, &config, sub_dt, &mut buffers);
    }

    world.insert_resource(buffers);
}

/// Substeps cap per-step travel by smallest dynamic half-extent.
fn compute_substeps(world: &World, dt: f32, config: &PhysicsConfig) -> u32 {
    let mut worst_ratio = 0.0f32;
    for (entity, _vel) in world.query::<Velocity>() {
        let is_dynamic = matches!(
            world.get::<RigidBody>(entity).map(|b| b.kind),
            Some(BodyKind::Dynamic)
        );
        if !is_dynamic {
            continue;
        }
        let Some(velocity) = world.get::<Velocity>(entity) else {
            continue;
        };
        let Some(collider) = world.get::<Collider>(entity) else {
            continue;
        };
        let min_extent = collider.shape.min_half_extent().max(0.5);
        let travel = velocity.0.length() * dt;
        let ratio = travel / min_extent;
        if ratio > worst_ratio {
            worst_ratio = ratio;
        }
    }
    let needed = worst_ratio.ceil().max(1.0) as u32;
    needed.min(config.max_substeps.max(1))
}

fn apply_gravity_and_integrate(
    world: &mut World,
    sub_dt: f32,
    gravity: Vec2,
    buffers: &mut PhysicsBuffers,
) {
    buffers.dynamic_entities.clear();
    for e in world.query_entities::<RigidBody>() {
        if matches!(
            world.get::<RigidBody>(e).map(|b| b.kind),
            Some(BodyKind::Dynamic)
        ) {
            buffers.dynamic_entities.push(e);
        }
    }

    for &entity in &buffers.dynamic_entities {
        if let Some(vel) = world.get_mut::<Velocity>(entity) {
            vel.0 += gravity * sub_dt;
        }
        let step = world
            .get::<Velocity>(entity)
            .map_or(Vec2::ZERO, |v| v.0 * sub_dt);
        if let Some(pos) = world.get_mut::<Position>(entity) {
            pos.0 += step;
        }
    }
}

fn resolve_collisions(
    world: &mut World,
    config: &PhysicsConfig,
    sub_dt: f32,
    buffers: &mut PhysicsBuffers,
) {
    let PhysicsBuffers {
        proxies,
        pairs,
        events,
        candidates,
        grid,
        collider_entities,
        ..
    } = buffers;
    proxies.clear();
    pairs.clear();
    events.clear();
    candidates.clear();
    collider_entities.clear();

    // Preserve grid allocations unless cell size changes.
    if (grid.cell_size() - config.broadphase_cell_size).abs() > f32::EPSILON {
        grid.set_cell_size(config.broadphase_cell_size);
    } else {
        grid.clear();
    }

    for e in world.query_entities::<Collider>() {
        if world.get::<Position>(e).is_some() {
            collider_entities.push(e);
        }
    }

    for entity in collider_entities.iter() {
        let position = world.get::<Position>(*entity).copied().unwrap().0;
        let collider = *world.get::<Collider>(*entity).unwrap();
        let body = world.get::<RigidBody>(*entity).copied();
        let velocity = world
            .get::<Velocity>(*entity)
            .copied()
            .map_or(Vec2::ZERO, |v| v.0);
        let (is_dynamic, inv_mass, restitution) = match body {
            Some(b) => (
                b.kind == BodyKind::Dynamic,
                if b.kind == BodyKind::Dynamic {
                    b.inv_mass
                } else {
                    0.0
                },
                b.restitution,
            ),
            None => (false, 0.0, 0.0),
        };
        let center = position + collider.offset;
        // Recover pre-integration center after `velocity * sub_dt` move.
        let prev_center = if is_dynamic {
            center - velocity * sub_dt
        } else {
            center
        };
        proxies.push(Proxy {
            entity: Some(*entity),
            center,
            prev_center,
            velocity,
            offset: collider.offset,
            shape: collider.shape,
            is_dynamic,
            inv_mass,
            restitution,
            face_mask: FACE_ALL,
        });
    }

    gather_tilemap_proxies(world, proxies);

    for (id, proxy) in proxies.iter().enumerate() {
        grid.insert(id as ProxyId, &proxy.world_aabb());
    }

    // Extreme tunneling guard: clamp cap-bound dynamics to first static sweep hit.
    speculative_pass(proxies, grid, candidates, events);

    // Pair query: swept AABB + half-cell margin covers tunneling and GS slip.
    let query_margin = Vec2::splat(config.broadphase_cell_size * 0.5);
    for a_idx in 0..proxies.len() {
        if !proxies[a_idx].is_dynamic {
            continue;
        }
        let mut a_aabb = proxies[a_idx].swept_aabb();
        a_aabb.half_extents += query_margin;
        grid.query(&a_aabb, Some(a_idx as ProxyId), candidates);
        for &b_id in candidates.iter() {
            let b_idx = b_id as usize;
            // Dynamic pairs once; static targets already unique per dynamic query.
            if proxies[b_idx].is_dynamic && b_idx <= a_idx {
                continue;
            }
            pairs.push((a_idx as u32, b_idx as u32));
        }
    }

    // GS solver: re-test corrected centers; emit events on iteration 0 only.
    let iterations = config.solver_iterations.max(1);
    for iteration in 0..iterations {
        let emit_events = iteration == 0;
        for &(a_u, b_u) in pairs.iter() {
            let a_idx = a_u as usize;
            let b_idx = b_u as usize;

            let contact = narrow_phase(&proxies[a_idx], &proxies[b_idx]);
            let Some(contact) = contact else { continue };

            let inv_mass_sum = proxies[a_idx].inv_mass + proxies[b_idx].inv_mass;
            if inv_mass_sum <= 0.0 {
                continue;
            }

            let a_share = proxies[a_idx].inv_mass / inv_mass_sum;
            let b_share = proxies[b_idx].inv_mass / inv_mass_sum;
            let correction = contact.normal * contact.penetration;

            // Closing velocity along `normal`: `(vb - va) dot n`.
            let a_vel = proxies[a_idx].velocity;
            let b_vel = proxies[b_idx].velocity;
            let relative = b_vel - a_vel;
            let vel_along_normal = relative.dot(contact.normal);
            let restitution = proxies[a_idx].restitution.max(proxies[b_idx].restitution);

            let (a_dv, b_dv) = if vel_along_normal <= 0.0 {
                (Vec2::ZERO, Vec2::ZERO)
            } else {
                let j = -(1.0 + restitution) * vel_along_normal / inv_mass_sum;
                let impulse = contact.normal * j;
                (
                    -impulse * proxies[a_idx].inv_mass,
                    impulse * proxies[b_idx].inv_mass,
                )
            };

            // In-place correction feeds subsequent contacts in this iteration.
            proxies[a_idx].center += correction * a_share;
            proxies[a_idx].velocity += a_dv;
            if proxies[b_idx].is_dynamic {
                proxies[b_idx].center -= correction * b_share;
                proxies[b_idx].velocity += b_dv;
            }

            if emit_events {
                if let Some(a_entity) = proxies[a_idx].entity {
                    events.push(CollisionEvent {
                        a: a_entity,
                        b: proxies[b_idx].entity,
                        normal: contact.normal,
                        penetration: contact.penetration,
                    });
                }
            }
        }
    }

    for proxy in proxies.iter() {
        if !proxy.is_dynamic {
            continue;
        }
        let Some(entity) = proxy.entity else { continue };
        if let Some(pos) = world.get_mut::<Position>(entity) {
            pos.0 = proxy.center - proxy.offset;
        }
        if let Some(vel) = world.get_mut::<Velocity>(entity) {
            vel.0 = proxy.velocity;
        }
    }

    // Drain preserves event buffer allocation.
    if !events.is_empty() {
        if let Some(sink) = world.get_resource_mut::<EventQueue<CollisionEvent>>() {
            for event in events.drain(..) {
                sink.send(event);
            }
        } else {
            events.clear();
        }
    }
}

/// Clamp fast dynamics to first static sweep hit; conservative against tunneling.
fn speculative_pass(
    proxies: &mut [Proxy],
    grid: &mut SpatialGrid,
    candidates: &mut Vec<ProxyId>,
    events: &mut Vec<CollisionEvent>,
) {
    const BACKOFF: f32 = 1.0e-3;

    for a_idx in 0..proxies.len() {
        let proxy = &proxies[a_idx];
        if !proxy.is_dynamic {
            continue;
        }
        let min_extent = proxy.min_half_extent();
        if min_extent <= 0.0 {
            continue;
        }
        let travel_sq = (proxy.center - proxy.prev_center).length_squared();
        // Same threshold as substep picker, squared.
        if travel_sq <= min_extent * min_extent {
            continue;
        }

        let swept = proxy.swept_aabb();
        grid.query(&swept, Some(a_idx as ProxyId), candidates);

        let a_prev = proxy.prev_center;
        let a_cur = proxy.center;
        let a_half = proxy.half_extents();

        let mut best: Option<(f32, Vec2, usize)> = None;
        for &b_id in candidates.iter() {
            let b_idx = b_id as usize;
            let target = &proxies[b_idx];
            if target.is_dynamic {
                continue;
            }
            let Shape::Aabb {
                half_extents: b_half,
            } = target.shape
            else {
                continue;
            };
            if let Some((t, n)) = sweep_aabb_vs_aabb(a_prev, a_cur, a_half, target.center, b_half) {
                // Skip hits on internal tile faces.
                let face_bit = if n.x < 0.0 {
                    FACE_LEFT
                } else if n.x > 0.0 {
                    FACE_RIGHT
                } else if n.y < 0.0 {
                    FACE_TOP
                } else {
                    FACE_BOTTOM
                };
                if target.face_mask & face_bit == 0 {
                    continue;
                }
                if best.is_none_or(|(best_t, _, _)| t < best_t) {
                    best = Some((t, n, b_idx));
                }
            }
        }

        let Some((t_hit, normal, b_idx)) = best else {
            continue;
        };
        let t_safe = (t_hit - BACKOFF).max(0.0);
        let clamped = a_prev + (a_cur - a_prev) * t_safe;
        proxies[a_idx].center = clamped;

        // Reflect closing velocity along contact normal.
        let v = proxies[a_idx].velocity;
        let v_along_normal = v.dot(normal);
        if v_along_normal < 0.0 {
            let restitution = proxies[a_idx].restitution;
            let impulse = -(1.0 + restitution) * v_along_normal;
            proxies[a_idx].velocity = v + normal * impulse;
        }

        if let Some(a_entity) = proxies[a_idx].entity {
            events.push(CollisionEvent {
                a: a_entity,
                b: proxies[b_idx].entity,
                normal,
                penetration: 0.0,
            });
        }
    }
}

fn narrow_phase(a: &Proxy, b: &Proxy) -> Option<Contact> {
    match (a.shape, b.shape) {
        (
            Shape::Aabb {
                half_extents: ha, ..
            },
            Shape::Aabb {
                half_extents: hb, ..
            },
        ) => aabb_vs_aabb_masked(
            &Aabb::new(a.center, ha),
            &Aabb::new(b.center, hb),
            b.face_mask,
        ),
        (Shape::Circle { radius: ra }, Shape::Circle { radius: rb }) => {
            circle_vs_circle(a.center, ra, b.center, rb)
        }
        (Shape::Aabb { half_extents }, Shape::Circle { radius }) => aabb_vs_circle_masked(
            &Aabb::new(a.center, half_extents),
            b.center,
            radius,
            a.face_mask,
        ),
        (Shape::Circle { radius }, Shape::Aabb { half_extents }) => {
            // Helper returns AABB escape normal; circle needs the opposite.
            aabb_vs_circle_masked(
                &Aabb::new(b.center, half_extents),
                a.center,
                radius,
                b.face_mask,
            )
            .map(|c| Contact {
                normal: -c.normal,
                penetration: c.penetration,
            })
        }
    }
}

/// Emit transient static tile proxies for collision layers.
fn gather_tilemap_proxies(world: &World, proxies: &mut Vec<Proxy>) {
    let Some(registry) = world.get_resource::<TilemapRegistry>() else {
        return;
    };

    for (_entity, instance) in world.query::<TilemapInstance>() {
        let Some(data) = registry.get(&instance.id) else {
            continue;
        };
        let tw = data.tile_width as f32;
        let th = data.tile_height as f32;
        let half = Vec2::new(tw * 0.5, th * 0.5);

        for layer in &data.layers {
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
                        center,
                        prev_center: center,
                        velocity: Vec2::ZERO,
                        offset: Vec2::ZERO,
                        shape: Shape::Aabb { half_extents: half },
                        is_dynamic: false,
                        inv_mass: 0.0,
                        restitution: 0.0,
                        face_mask,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "../tests/physics/step.rs"]
mod tests;
