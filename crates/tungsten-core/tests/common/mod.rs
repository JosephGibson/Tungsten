//! Spawn helpers shared by the release physics tests (`physics_determinism`,
//! `physics_containment`): the bench's dense pile and the static box around it.

use glam::Vec2;
use tungsten_core::{Collider, Entity, Pcg32, Position, RigidBody, Velocity, World};

pub const BODY_RADIUS: f32 = 6.0;
pub const SPAWN_SPACING: f32 = 14.0;
pub const PILE_WIDTH: f32 = 1_920.0;
pub const FLOOR_Y: f32 = 1_080.0;

/// Fully closed box (floor, roof, side walls) spanning `top_y..FLOOR_Y`, with
/// walls `2 * wall_half` thick; mirrors the bench geometry.
pub fn spawn_static_box(world: &mut World, width: f32, top_y: f32, wall_half: f32) {
    let mid_x = width * 0.5;
    let mid_y = f32::midpoint(top_y, FLOOR_Y);
    let half_h = (FLOOR_Y - top_y) * 0.5;
    let walls = [
        (
            Vec2::new(mid_x, FLOOR_Y + wall_half),
            Vec2::new(mid_x + wall_half * 2.0, wall_half),
        ),
        (
            Vec2::new(mid_x, top_y - wall_half),
            Vec2::new(mid_x + wall_half * 2.0, wall_half),
        ),
        (
            Vec2::new(-wall_half, mid_y),
            Vec2::new(wall_half, half_h + wall_half * 2.0),
        ),
        (
            Vec2::new(width + wall_half, mid_y),
            Vec2::new(wall_half, half_h + wall_half * 2.0),
        ),
    ];
    for (center, half_extents) in walls {
        let entity = world.spawn();
        world.insert(entity, Position(center - half_extents));
        world.insert(entity, RigidBody::r#static());
        world.insert(
            entity,
            Collider::aabb(half_extents).with_offset(half_extents),
        );
    }
}

/// `count` dynamic circles in jittered rows resting above `FLOOR_Y`, in spawn
/// order.
pub fn spawn_pile(world: &mut World, count: usize, rng: &mut Pcg32) -> Vec<Entity> {
    let usable_width = PILE_WIDTH - SPAWN_SPACING * 2.0;
    let cols = ((usable_width / SPAWN_SPACING).floor() as usize).max(1);
    let rows = count.div_ceil(cols);
    let start_y = FLOOR_Y - SPAWN_SPACING - rows as f32 * SPAWN_SPACING;

    let mut bodies = Vec::with_capacity(count);
    for index in 0..count {
        let col = index % cols;
        let row = index / cols;
        let jitter_x = rng.next_range(-0.2, 0.2) * SPAWN_SPACING;
        let jitter_y = rng.next_range(-0.2, 0.2) * SPAWN_SPACING;
        let x = (SPAWN_SPACING + col as f32 * SPAWN_SPACING + jitter_x)
            .clamp(0.0, PILE_WIDTH - BODY_RADIUS * 2.0);
        let y = start_y + row as f32 * SPAWN_SPACING + jitter_y;
        let entity = world.spawn();
        world.insert(entity, Position(Vec2::new(x, y)));
        world.insert(entity, Velocity(Vec2::ZERO));
        world.insert(entity, RigidBody::dynamic().with_restitution(0.1));
        world.insert(
            entity,
            Collider::circle(BODY_RADIUS).with_offset(Vec2::splat(BODY_RADIUS)),
        );
        bodies.push(entity);
    }
    bodies
}
