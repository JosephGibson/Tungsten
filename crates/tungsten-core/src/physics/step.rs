//! The physics step system. Registered with `app.add_system(physics_step)`
//! after gameplay movement input and before render extract.
//!
//! Per frame:
//!
//!   1. Populate `EventQueue<CollisionEvent>` with resolved contacts.
//!   2. Decide substep count from max dynamic speed and min half-extent.
//!   3. For each substep: apply gravity, integrate positions, rebuild
//!      the broad-phase grid from scratch, emit tilemap static colliders
//!      into the same grid, run narrow-phase against every dynamic proxy,
//!      resolve via minimum-translation-vector and push one event per
//!      contact.
//!
//! There is deliberately no persistent broad-phase state between
//! frames — rebuilding the grid from scratch is cheap at Phase 2 scale
//! and sidesteps incremental-update bugs.

use super::broadphase::{ProxyId, SpatialGrid};
use super::collision::{aabb_vs_aabb, aabb_vs_circle, circle_vs_circle, Aabb, Contact};
use super::components::{BodyKind, Collider, Position, RigidBody, Shape, Velocity};
use super::events::CollisionEvent;
use super::PhysicsConfig;
use crate::assets::{LayerKind, TilemapInstance, TilemapRegistry};
use crate::ecs::{Entity, EventQueue, World};
use crate::time::DeltaTime;
use glam::Vec2;

/// Internal per-step proxy record. One per dynamic/static collider,
/// including tilemap tiles (which carry `entity = None`). Mutated
/// in-place during resolution so sequential contacts see the
/// already-corrected state (avoids stacking impulses when a body
/// straddles two adjacent static tiles).
#[derive(Debug, Clone, Copy)]
struct Proxy {
    entity: Option<Entity>,
    center: Vec2,
    velocity: Vec2,
    offset: Vec2,
    shape: Shape,
    is_dynamic: bool,
    inv_mass: f32,
    restitution: f32,
}

impl Proxy {
    fn world_aabb(&self) -> Aabb {
        match self.shape {
            Shape::Aabb { half_extents } => Aabb::new(self.center, half_extents),
            Shape::Circle { radius } => Aabb::new(self.center, Vec2::splat(radius)),
        }
    }
}

/// Entry point registered with `app.add_system`. Runs one physics tick
/// with N substeps, where N is chosen to avoid tunneling at current
/// velocity.
pub fn physics_step(world: &mut World) {
    let dt = world
        .get_resource::<DeltaTime>()
        .map(|d| d.seconds())
        .unwrap_or(0.0);
    if dt <= 0.0 {
        return;
    }

    let config = world
        .get_resource::<PhysicsConfig>()
        .copied()
        .unwrap_or_default();

    let substeps = compute_substeps(world, dt, &config);
    let sub_dt = dt / substeps as f32;

    for _ in 0..substeps {
        apply_gravity_and_integrate(world, sub_dt, config.gravity);
        resolve_collisions(world, &config);
    }
}

/// Decide how many substeps this frame needs so that no dynamic body
/// moves further than its smallest half-extent in one integration.
/// Capped at `config.max_substeps`.
fn compute_substeps(world: &World, dt: f32, config: &PhysicsConfig) -> u32 {
    let mut worst_ratio = 0.0f32;
    for (entity, _vel) in world.query::<Velocity>() {
        // Only dynamic bodies contribute to the substep calculation;
        // static bodies don't move, tile colliders are derived.
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

fn apply_gravity_and_integrate(world: &mut World, sub_dt: f32, gravity: Vec2) {
    let dynamic_entities: Vec<Entity> = world
        .query_entities::<RigidBody>()
        .into_iter()
        .filter(|e| {
            matches!(
                world.get::<RigidBody>(*e).map(|b| b.kind),
                Some(BodyKind::Dynamic)
            )
        })
        .collect();

    for entity in dynamic_entities {
        if let Some(vel) = world.get_mut::<Velocity>(entity) {
            vel.0 += gravity * sub_dt;
        }
        let step = world
            .get::<Velocity>(entity)
            .map(|v| v.0 * sub_dt)
            .unwrap_or(Vec2::ZERO);
        if let Some(pos) = world.get_mut::<Position>(entity) {
            pos.0 += step;
        }
    }
}

fn resolve_collisions(world: &mut World, config: &PhysicsConfig) {
    let mut proxies: Vec<Proxy> = Vec::new();

    // 1. Collect entity proxies.
    let entities_with_collider: Vec<Entity> = world
        .query_entities::<Collider>()
        .into_iter()
        .filter(|e| world.get::<Position>(*e).is_some())
        .collect();

    for entity in &entities_with_collider {
        let position = world.get::<Position>(*entity).copied().unwrap().0;
        let collider = *world.get::<Collider>(*entity).unwrap();
        let body = world.get::<RigidBody>(*entity).copied();
        let velocity = world
            .get::<Velocity>(*entity)
            .copied()
            .map(|v| v.0)
            .unwrap_or(Vec2::ZERO);
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
            // No RigidBody → treat as a static collider for the purpose
            // of the query, but it can't generate events as `a`.
            None => (false, 0.0, 0.0),
        };
        proxies.push(Proxy {
            entity: Some(*entity),
            center: position + collider.offset,
            velocity,
            offset: collider.offset,
            shape: collider.shape,
            is_dynamic,
            inv_mass,
            restitution,
        });
    }

    // 2. Emit tilemap static tile proxies.
    gather_tilemap_proxies(world, &mut proxies);

    // 3. Build the broad-phase grid.
    let mut grid = SpatialGrid::new(config.broadphase_cell_size);
    for (id, proxy) in proxies.iter().enumerate() {
        grid.insert(id as ProxyId, &proxy.world_aabb());
    }

    // 4. For each dynamic proxy, query the grid and run narrow-phase.
    //    Contacts are resolved *sequentially* into `proxies`: each pair
    //    re-tests the narrow phase against the current (already-corrected)
    //    centers and reads the current velocities, so overlapping contacts
    //    (a body straddling two adjacent static tiles) don't stack
    //    impulses. Gauss–Seidel style — the ordering isn't physically
    //    unique but at game-jam scale the difference is invisible, and
    //    the alternative (batched deltas) doubles bounces on shared seams.
    let mut events: Vec<CollisionEvent> = Vec::new();
    let mut candidates: Vec<ProxyId> = Vec::new();

    for a_idx in 0..proxies.len() {
        if !proxies[a_idx].is_dynamic {
            continue;
        }
        let a_aabb = proxies[a_idx].world_aabb();
        grid.query(&a_aabb, Some(a_idx as ProxyId), &mut candidates);

        for &b_id in &candidates {
            let b_idx = b_id as usize;
            // Resolve each unordered pair once per substep: when both
            // sides are dynamic, only process when a_idx < b_idx.
            if proxies[b_idx].is_dynamic && b_idx <= a_idx {
                continue;
            }

            // Re-test narrow phase with current (possibly already
            // corrected) centers. If a prior contact in this substep
            // already separated the shapes, this returns None.
            let contact = narrow_phase(&proxies[a_idx], &proxies[b_idx]);
            let Some(contact) = contact else { continue };

            let inv_mass_sum = proxies[a_idx].inv_mass + proxies[b_idx].inv_mass;
            if inv_mass_sum <= 0.0 {
                continue;
            }

            // Positional correction: split along inverse-mass ratio.
            let a_share = proxies[a_idx].inv_mass / inv_mass_sum;
            let b_share = proxies[b_idx].inv_mass / inv_mass_sum;
            let correction = contact.normal * contact.penetration;

            // Velocity resolution: projection along contact normal with
            // combined restitution. `normal` points from a toward a's
            // free space, so relative velocity along the normal closing
            // the gap is `(vb - va) · n`.
            let a_vel = proxies[a_idx].velocity;
            let b_vel = proxies[b_idx].velocity;
            let relative = b_vel - a_vel;
            let vel_along_normal = relative.dot(contact.normal);
            let restitution = proxies[a_idx].restitution.max(proxies[b_idx].restitution);

            let (a_dv, b_dv) = if vel_along_normal <= 0.0 {
                // Bodies already separating (or tangent) — don't add
                // impulse, but still correct positions so resting
                // contacts don't accumulate penetration.
                (Vec2::ZERO, Vec2::ZERO)
            } else {
                let j = -(1.0 + restitution) * vel_along_normal / inv_mass_sum;
                let impulse = contact.normal * j;
                (
                    -impulse * proxies[a_idx].inv_mass,
                    impulse * proxies[b_idx].inv_mass,
                )
            };

            // Apply in-place so the next contact on this proxy sees the
            // corrected state. Static `b` proxies are never mutated.
            proxies[a_idx].center += correction * a_share;
            proxies[a_idx].velocity += a_dv;
            if proxies[b_idx].is_dynamic {
                proxies[b_idx].center -= correction * b_share;
                proxies[b_idx].velocity += b_dv;
            }

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

    // 5. Write the resolved proxy state back into the world.
    for proxy in &proxies {
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

    // 6. Push events into the resource.
    if !events.is_empty() {
        if let Some(sink) = world.get_resource_mut::<EventQueue<CollisionEvent>>() {
            for event in events {
                sink.send(event);
            }
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
        ) => aabb_vs_aabb(&Aabb::new(a.center, ha), &Aabb::new(b.center, hb)),
        (Shape::Circle { radius: ra }, Shape::Circle { radius: rb }) => {
            circle_vs_circle(a.center, ra, b.center, rb)
        }
        (Shape::Aabb { half_extents }, Shape::Circle { radius }) => {
            aabb_vs_circle(&Aabb::new(a.center, half_extents), b.center, radius)
        }
        (Shape::Circle { radius }, Shape::Aabb { half_extents }) => {
            // Helper's `a` is our b (the aabb). Its returned normal is the
            // direction the aabb escapes the circle, which is exactly the
            // opposite of the direction our `a` (the circle) needs to move.
            aabb_vs_circle(&Aabb::new(b.center, half_extents), a.center, radius).map(|c| Contact {
                normal: -c.normal,
                penetration: c.penetration,
            })
        }
    }
}

/// Walk every `TilemapInstance` entity and emit a static AABB proxy
/// per non-negative tile on any `LayerKind::Collision` layer. These
/// proxies are transient — generated fresh each substep so hot-reloaded
/// collision layers take effect on the next frame.
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
            for row in 0..data.height {
                for col in 0..data.width {
                    let idx = (row as usize) * (data.width as usize) + (col as usize);
                    let tile = layer.tiles[idx];
                    if tile < 0 {
                        continue;
                    }
                    let center = Vec2::new(
                        instance.origin.x + (col as f32) * tw + half.x,
                        instance.origin.y + (row as f32) * th + half.y,
                    );
                    proxies.push(Proxy {
                        entity: None,
                        center,
                        velocity: Vec2::ZERO,
                        offset: Vec2::ZERO,
                        shape: Shape::Aabb { half_extents: half },
                        is_dynamic: false,
                        inv_mass: 0.0,
                        restitution: 0.0,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::{TilemapData, TilemapLayer, TilemapRegistry};
    use crate::ecs::World;

    fn seed_world() -> World {
        let mut world = World::new();
        world.insert_resource(DeltaTime { dt: 1.0 / 60.0 });
        world.insert_resource(EventQueue::<CollisionEvent>::new());
        world.insert_resource(PhysicsConfig::default());
        world.insert_resource(TilemapRegistry::new());
        world
    }

    #[test]
    fn integrates_dynamic_position_from_velocity() {
        let mut world = seed_world();
        let e = world.spawn();
        world.insert(e, Position(Vec2::new(0.0, 0.0)));
        world.insert(e, Velocity(Vec2::new(60.0, 0.0)));
        world.insert(e, Collider::aabb(Vec2::new(8.0, 8.0)));
        world.insert(e, RigidBody::dynamic());

        physics_step(&mut world);

        let pos = world.get::<Position>(e).unwrap();
        assert!((pos.0.x - 1.0).abs() < 1e-3, "got {:?}", pos.0);
    }

    #[test]
    fn dynamic_aabb_resolves_against_static_aabb() {
        let mut world = seed_world();

        // Dynamic at origin moving right; static block right of it.
        let dynamic = world.spawn();
        world.insert(dynamic, Position(Vec2::new(0.0, 0.0)));
        world.insert(dynamic, Velocity(Vec2::new(600.0, 0.0)));
        world.insert(dynamic, Collider::aabb(Vec2::new(8.0, 8.0)));
        world.insert(dynamic, RigidBody::dynamic());

        let wall = world.spawn();
        world.insert(wall, Position(Vec2::new(32.0, 0.0)));
        world.insert(wall, Collider::aabb(Vec2::new(8.0, 32.0)));
        world.insert(wall, RigidBody::r#static());

        // Run a long enough time to try to tunnel in a single integrate.
        if let Some(dt) = world.get_resource_mut::<DeltaTime>() {
            dt.dt = 0.1;
        }
        physics_step(&mut world);

        let pos = world.get::<Position>(dynamic).unwrap();
        // Centered AABBs: dynamic right edge must not cross wall left edge.
        assert!(
            pos.0.x + 8.0 <= 32.0 - 8.0 + 1e-3,
            "penetrated: {:?}",
            pos.0
        );
        let events = world.get_resource::<EventQueue<CollisionEvent>>().unwrap();
        assert!(!events.is_empty(), "expected at least one collision event");
    }

    #[test]
    fn tilemap_collision_layer_blocks_dynamic_body() {
        let mut world = seed_world();

        // 3x1 tilemap with a single solid tile at col=2, row=0.
        let registry = world.get_resource_mut::<TilemapRegistry>().unwrap();
        registry.insert(
            "test".into(),
            TilemapData {
                tile_width: 16,
                tile_height: 16,
                width: 3,
                height: 1,
                tileset: vec!["solid".into()],
                layers: vec![TilemapLayer {
                    name: "solid".into(),
                    kind: LayerKind::Collision,
                    tiles: vec![-1, -1, 0],
                }],
            },
        );

        let map_e = world.spawn();
        world.insert(map_e, TilemapInstance::new("test", Vec2::ZERO));

        let player = world.spawn();
        world.insert(player, Position(Vec2::new(8.0 + 7.0, 8.0)));
        world.insert(player, Velocity(Vec2::new(600.0, 0.0)));
        world.insert(player, Collider::aabb(Vec2::new(7.0, 7.0)));
        world.insert(player, RigidBody::dynamic());

        if let Some(dt) = world.get_resource_mut::<DeltaTime>() {
            dt.dt = 0.05;
        }
        physics_step(&mut world);

        let pos = world.get::<Position>(player).unwrap();
        // Solid tile spans x in [32, 48]. Player half-extent 7 means
        // the center must stay ≤ 32 - 7 = 25.
        assert!(pos.0.x <= 25.0 + 1e-3, "penetrated tile: {:?}", pos.0);
        let events = world.get_resource::<EventQueue<CollisionEvent>>().unwrap();
        assert!(events.iter_any_tile(), "expected a tile collision event");
    }

    #[test]
    fn circle_against_static_aabb_pushes_out() {
        let mut world = seed_world();

        let circle = world.spawn();
        world.insert(circle, Position(Vec2::new(0.0, 0.0)));
        world.insert(circle, Velocity(Vec2::new(-200.0, 0.0)));
        world.insert(circle, Collider::circle(4.0));
        world.insert(circle, RigidBody::dynamic());

        let wall = world.spawn();
        world.insert(wall, Position(Vec2::new(-8.0, 0.0)));
        world.insert(wall, Collider::aabb(Vec2::new(4.0, 16.0)));
        world.insert(wall, RigidBody::r#static());

        physics_step(&mut world);

        let pos = world.get::<Position>(circle).unwrap();
        // Circle must not penetrate into wall (wall right edge = -4.0;
        // circle center must stay ≥ 0.0).
        assert!(pos.0.x >= 0.0 - 1e-3, "penetrated wall: {:?}", pos.0);
    }

    #[test]
    fn substep_count_prevents_tunneling_of_fast_body() {
        let mut world = seed_world();
        // Big dt + high velocity → would tunnel without substeps.
        if let Some(dt) = world.get_resource_mut::<DeltaTime>() {
            dt.dt = 1.0 / 30.0;
        }
        let dynamic = world.spawn();
        world.insert(dynamic, Position(Vec2::new(0.0, 0.0)));
        world.insert(dynamic, Velocity(Vec2::new(2000.0, 0.0)));
        world.insert(dynamic, Collider::aabb(Vec2::new(4.0, 4.0)));
        world.insert(dynamic, RigidBody::dynamic());

        let wall = world.spawn();
        world.insert(wall, Position(Vec2::new(40.0, 0.0)));
        world.insert(wall, Collider::aabb(Vec2::new(4.0, 32.0)));
        world.insert(wall, RigidBody::r#static());

        physics_step(&mut world);

        let pos = world.get::<Position>(dynamic).unwrap();
        assert!(pos.0.x + 4.0 <= 40.0 - 4.0 + 1e-3, "tunneled: {:?}", pos.0);
    }

    #[test]
    fn zero_restitution_body_does_not_bounce_off_multi_tile_floor() {
        // Regression: the old batched delta resolver summed one full
        // impulse per contact, so a body straddling two adjacent floor
        // tiles bounced upward at its incoming speed even with
        // restitution 0. This test pins sequential resolution in place.
        let mut world = seed_world();
        world.get_resource_mut::<TilemapRegistry>().unwrap().insert(
            "floor".into(),
            TilemapData {
                tile_width: 16,
                tile_height: 16,
                width: 4,
                height: 1,
                tileset: vec!["solid".into()],
                layers: vec![TilemapLayer {
                    name: "collision".into(),
                    kind: LayerKind::Collision,
                    tiles: vec![0, 0, 0, 0],
                }],
            },
        );
        let map = world.spawn();
        world.insert(map, TilemapInstance::new("floor", Vec2::new(0.0, 16.0)));

        // Player straddles the seam between tile cols 0 and 1.
        let player = world.spawn();
        world.insert(player, Position(Vec2::new(16.0, 9.0)));
        world.insert(player, Velocity(Vec2::new(0.0, 50.0)));
        world.insert(player, Collider::aabb(Vec2::new(6.0, 7.0)));
        world.insert(player, RigidBody::dynamic().with_restitution(0.0));

        physics_step(&mut world);

        let vel = world.get::<Velocity>(player).unwrap().0;
        assert!(
            vel.y >= -1e-3,
            "zero-restitution body bounced upward off flat floor: {:?}",
            vel
        );
    }

    #[test]
    fn bouncy_ball_does_not_double_impulse_on_multi_tile_seam() {
        // Same bug as above but with restitution 0.85: the ball should
        // rebound at 0.85× its incoming vertical speed, not 2–3×.
        let mut world = seed_world();
        world.get_resource_mut::<TilemapRegistry>().unwrap().insert(
            "floor".into(),
            TilemapData {
                tile_width: 16,
                tile_height: 16,
                width: 4,
                height: 1,
                tileset: vec!["solid".into()],
                layers: vec![TilemapLayer {
                    name: "collision".into(),
                    kind: LayerKind::Collision,
                    tiles: vec![0, 0, 0, 0],
                }],
            },
        );
        let map = world.spawn();
        world.insert(map, TilemapInstance::new("floor", Vec2::new(0.0, 16.0)));

        let ball = world.spawn();
        // Centered on the seam between tile cols 0 and 1, above the floor.
        world.insert(ball, Position(Vec2::new(16.0, 9.0)));
        world.insert(ball, Velocity(Vec2::new(0.0, 50.0)));
        world.insert(ball, Collider::aabb(Vec2::new(6.0, 6.0)));
        world.insert(ball, RigidBody::dynamic().with_restitution(0.85));

        physics_step(&mut world);

        let vel = world.get::<Velocity>(ball).unwrap().0;
        // Incoming v_y = 50 + gravity*dt. With restitution 0.85 the
        // post-bounce speed along +y (downward) must be less than the
        // incoming speed. The pre-fix bug produced |v_y| well above
        // the incoming speed (≈2–3× amplification).
        assert!(
            vel.y > -60.0,
            "ball impulse was doubled — rebounded too fast: {:?}",
            vel
        );
    }

    // Helper extension used by tests.
    trait CollisionEventQueueExt {
        fn iter_any_tile(&self) -> bool;
    }

    impl CollisionEventQueueExt for EventQueue<CollisionEvent> {
        fn iter_any_tile(&self) -> bool {
            self.iter_current().any(|e| e.b.is_none())
        }
    }
}
