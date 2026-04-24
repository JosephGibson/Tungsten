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

    let dynamic = world.spawn();
    world.insert(dynamic, Position(Vec2::new(0.0, 0.0)));
    world.insert(dynamic, Velocity(Vec2::new(600.0, 0.0)));
    world.insert(dynamic, Collider::aabb(Vec2::new(8.0, 8.0)));
    world.insert(dynamic, RigidBody::dynamic());

    let wall = world.spawn();
    world.insert(wall, Position(Vec2::new(32.0, 0.0)));
    world.insert(wall, Collider::aabb(Vec2::new(8.0, 32.0)));
    world.insert(wall, RigidBody::r#static());

    if let Some(dt) = world.get_resource_mut::<DeltaTime>() {
        dt.dt = 0.1;
    }
    physics_step(&mut world);

    let pos = world.get::<Position>(dynamic).unwrap();
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
    // Solid tile x=[32,48]; player center <= 25.
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
    assert!(pos.0.x >= 0.0 - 1e-3, "penetrated wall: {:?}", pos.0);
}

#[test]
fn substep_count_prevents_tunneling_of_fast_body() {
    let mut world = seed_world();
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
fn inflated_broadphase_catches_resolution_slip_into_unpaired_wall() {
    // Regression: GS slip crosses cell boundary before re-pairing; half-cell margin catches it.
    let mut world = seed_world();
    if let Some(cfg) = world.get_resource_mut::<PhysicsConfig>() {
        cfg.broadphase_cell_size = 16.0;
        cfg.max_substeps = 1;
    }

    // Wall x=[18,22], cell 1.
    let wall = world.spawn();
    world.insert(wall, Position(Vec2::new(20.0, 0.0)));
    world.insert(wall, Collider::aabb(Vec2::new(2.0, 8.0)));
    world.insert(wall, RigidBody::r#static());

    // Heavy shover travels 500/60 px in one capped substep.
    let shover = world.spawn();
    world.insert(shover, Position(Vec2::new(0.0, 0.0)));
    world.insert(shover, Velocity(Vec2::new(500.0, 0.0)));
    world.insert(shover, Collider::circle(4.0));
    world.insert(
        shover,
        RigidBody {
            kind: BodyKind::Dynamic,
            inv_mass: 0.1,
            restitution: 0.0,
        },
    );

    // Target ball x=[6,14], cell 0.
    let ball = world.spawn();
    world.insert(ball, Position(Vec2::new(10.0, 0.0)));
    world.insert(ball, Velocity(Vec2::ZERO));
    world.insert(ball, Collider::circle(4.0));
    world.insert(ball, RigidBody::dynamic().with_restitution(0.0));

    if let Some(dt) = world.get_resource_mut::<DeltaTime>() {
        dt.dt = 1.0 / 60.0;
    }
    physics_step(&mut world);

    // Wall left face x=18; ball center <=14.
    let ball_pos = world.get::<Position>(ball).unwrap().0;
    assert!(
        ball_pos.x <= 14.0 + 0.2,
        "ball slipped through wall during GS resolution: {ball_pos:?}"
    );
}

#[test]
fn speculative_pass_prevents_tunneling_when_substep_cap_binds() {
    // Regression: cap-bound sub_dt misses thin wall; speculative pass clamps.
    let mut world = seed_world();
    if let Some(cfg) = world.get_resource_mut::<PhysicsConfig>() {
        cfg.max_substeps = 1;
    }
    if let Some(dt) = world.get_resource_mut::<DeltaTime>() {
        dt.dt = 1.0 / 30.0;
    }

    let ball = world.spawn();
    world.insert(ball, Position(Vec2::new(0.0, 0.0)));
    // 4000 px/s over sub_dt ~= 133 px; wall thickness 8.
    world.insert(ball, Velocity(Vec2::new(4000.0, 0.0)));
    world.insert(ball, Collider::circle(4.0));
    world.insert(ball, RigidBody::dynamic().with_restitution(0.5));

    let wall = world.spawn();
    world.insert(wall, Position(Vec2::new(40.0, 0.0)));
    world.insert(wall, Collider::aabb(Vec2::new(4.0, 32.0)));
    world.insert(wall, RigidBody::r#static());

    physics_step(&mut world);

    let pos = world.get::<Position>(ball).unwrap();
    // Wall left face x=36; ball must not clear right face x=44.
    assert!(
        pos.0.x <= 36.0 + 0.5,
        "ball tunneled through wall despite speculative pass: {:?}",
        pos.0
    );
    let vel = world.get::<Velocity>(ball).unwrap().0;
    assert!(
        vel.x < 0.0,
        "velocity should reflect off wall under speculative: {vel:?}"
    );
}

#[test]
fn zero_restitution_body_does_not_bounce_off_multi_tile_floor() {
    // Regression: multi-tile contact must not sum duplicate impulses.
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

    let player = world.spawn();
    world.insert(player, Position(Vec2::new(16.0, 9.0)));
    world.insert(player, Velocity(Vec2::new(0.0, 50.0)));
    world.insert(player, Collider::aabb(Vec2::new(6.0, 7.0)));
    world.insert(player, RigidBody::dynamic().with_restitution(0.0));

    physics_step(&mut world);

    let vel = world.get::<Velocity>(player).unwrap().0;
    assert!(
        vel.y >= -1e-3,
        "zero-restitution body bounced upward off flat floor: {vel:?}"
    );
}

#[test]
fn bouncy_ball_does_not_double_impulse_on_multi_tile_seam() {
    // Regression: restitution applies once across tile seam contacts.
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
    world.insert(ball, Position(Vec2::new(16.0, 9.0)));
    world.insert(ball, Velocity(Vec2::new(0.0, 50.0)));
    world.insert(ball, Collider::aabb(Vec2::new(6.0, 6.0)));
    world.insert(ball, RigidBody::dynamic().with_restitution(0.85));

    physics_step(&mut world);

    let vel = world.get::<Velocity>(ball).unwrap().0;
    // Bound catches old 2x-3x seam amplification.
    assert!(
        vel.y > -60.0,
        "ball impulse was doubled — rebounded too fast: {vel:?}"
    );
}

#[test]
fn stack_of_dynamic_bodies_does_not_tunnel_static_floor() {
    // Regression: multi-iteration GS propagates stack pressure to floor contacts.
    let mut world = seed_world();
    if let Some(cfg) = world.get_resource_mut::<PhysicsConfig>() {
        cfg.gravity = Vec2::new(0.0, 900.0);
    }

    let floor = world.spawn();
    world.insert(floor, Position(Vec2::new(64.0, 108.0)));
    world.insert(floor, Collider::aabb(Vec2::new(64.0, 8.0)));
    world.insert(floor, RigidBody::r#static());

    // Pre-separated stack above floor.
    const BALLS: u32 = 4;
    const RADIUS: f32 = 4.0;
    let mut ball_entities = Vec::new();
    for i in 0..BALLS {
        let e = world.spawn();
        let y = 100.0 - 8.0 - RADIUS - (i as f32) * (RADIUS * 2.0 + 0.5);
        world.insert(e, Position(Vec2::new(64.0, y)));
        world.insert(e, Velocity(Vec2::ZERO));
        world.insert(e, Collider::circle(RADIUS));
        world.insert(e, RigidBody::dynamic().with_restitution(0.3));
        ball_entities.push(e);
    }

    for _ in 0..120 {
        physics_step(&mut world);
        world
            .get_resource_mut::<EventQueue<CollisionEvent>>()
            .unwrap()
            .flush();
    }

    // Floor top y=100; ball top <= 100 plus tolerance.
    let floor_top = 100.0;
    for (i, &ball) in ball_entities.iter().enumerate() {
        let pos = world.get::<Position>(ball).unwrap().0;
        assert!(
            pos.y + RADIUS <= floor_top + 0.5,
            "ball {i} clipped through floor: y={} (top={})",
            pos.y,
            pos.y + RADIUS
        );
    }
}

#[test]
fn pile_of_balls_does_not_escape_bottom_right_corner() {
    // Regression: pile pressure at L-corner seam must not escape tilemap.
    let mut world = seed_world();
    if let Some(cfg) = world.get_resource_mut::<PhysicsConfig>() {
        cfg.gravity = Vec2::new(0.0, 900.0);
        cfg.broadphase_cell_size = 16.0;
    }

    const W: u32 = 48;
    const H: u32 = 18;
    let mut tiles = vec![-1i32; (W * H) as usize];
    for row in 0..H {
        for col in 0..W {
            let solid = col == 0 || col == W - 1 || row == H - 2 || row == H - 1;
            if solid {
                tiles[(row * W + col) as usize] = 0;
            }
        }
    }
    world.get_resource_mut::<TilemapRegistry>().unwrap().insert(
        "level".into(),
        TilemapData {
            tile_width: 16,
            tile_height: 16,
            width: W,
            height: H,
            tileset: vec!["solid".into()],
            layers: vec![TilemapLayer {
                name: "collision".into(),
                kind: LayerKind::Collision,
                tiles,
            }],
        },
    );
    let map = world.spawn();
    world.insert(map, TilemapInstance::new("level", Vec2::ZERO));

    // Dense pile above bottom-right inside corner.
    const RADIUS: f32 = 6.0;
    const COLS: u32 = 6;
    const ROWS: u32 = 8;
    let wall_inner_x = (W - 1) as f32 * 16.0;
    let floor_top_y = (H - 2) as f32 * 16.0;
    let mut ball_entities = Vec::new();
    for row in 0..ROWS {
        for col in 0..COLS {
            let x = wall_inner_x - RADIUS - (col as f32) * (RADIUS * 2.0 + 0.25);
            let y = floor_top_y - RADIUS - (row as f32) * (RADIUS * 2.0 + 0.25);
            let e = world.spawn();
            world.insert(e, Position(Vec2::new(x, y)));
            world.insert(e, Velocity(Vec2::ZERO));
            world.insert(e, Collider::circle(RADIUS));
            world.insert(e, RigidBody::dynamic().with_restitution(0.85));
            ball_entities.push(e);
        }
    }

    for _ in 0..240 {
        physics_step(&mut world);
        world
            .get_resource_mut::<EventQueue<CollisionEvent>>()
            .unwrap()
            .flush();
    }

    for (i, &ball) in ball_entities.iter().enumerate() {
        let pos = world.get::<Position>(ball).unwrap().0;
        assert!(
            pos.x <= wall_inner_x + 0.5,
            "ball {i} escaped past right wall (x = {}, wall = {}), full pos {:?}",
            pos.x,
            wall_inner_x,
            pos,
        );
        assert!(
            pos.y <= floor_top_y + 0.5,
            "ball {i} escaped below floor (y = {}, floor = {})",
            pos.y,
            floor_top_y,
        );
    }
}

trait CollisionEventQueueExt {
    fn iter_any_tile(&self) -> bool;
}

impl CollisionEventQueueExt for EventQueue<CollisionEvent> {
    fn iter_any_tile(&self) -> bool {
        self.iter_current().any(|e| e.b.is_none())
    }
}
