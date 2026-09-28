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

    // Soft solver (D-063): approach velocity dies immediately; the residual
    // overlap recovers at the bias rate over subsequent steps instead of one
    // MTV push, settling at ~linear_slop.
    if let Some(dt) = world.get_resource_mut::<DeltaTime>() {
        dt.dt = 1.0 / 60.0;
    }
    for _ in 0..120 {
        physics_step(&mut world);
    }

    let pos = world.get::<Position>(dynamic).unwrap();
    assert!(pos.0.x + 8.0 <= 32.0 - 8.0 + 0.5, "penetrated: {:?}", pos.0);
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
    // First step kills the approach; the soft bias then recovers the
    // residual overlap toward linear_slop over subsequent steps (D-063).
    for _ in 0..30 {
        physics_step(&mut world);
    }

    let pos = world.get::<Position>(player).unwrap();
    // Solid tile x=[32,48]; player center <= 25 plus recovering overlap.
    assert!(pos.0.x <= 25.0 + 1.0, "penetrated tile: {:?}", pos.0);
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

    // Soft solver: push-out is bias-rate-limited and rests at ~linear_slop
    // instead of resolving in one MTV step (D-063).
    for _ in 0..90 {
        physics_step(&mut world);
    }

    let pos = world.get::<Position>(circle).unwrap();
    assert!(pos.0.x >= -0.5, "penetrated wall: {:?}", pos.0);
    assert!(pos.0.x <= 1.0, "overshot push-out: {:?}", pos.0);
}

#[test]
fn fast_body_stops_at_wall_under_fixed_substeps() {
    // Speculative contacts (D-064): 2000 px/s at dt 1/30 travels ~16.7 px per
    // fixed substep — well past the 4 px half-extent the old heuristic keyed
    // on. The gap contact clamps arrival at touching (rest ~linear_slop).
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
    assert!(pos.0.x + 4.0 <= 40.0 - 4.0 + 0.5, "tunneled: {:?}", pos.0);
}

#[test]
fn inflated_broadphase_catches_resolution_slip_into_unpaired_wall() {
    // Regression: GS slip crosses cell boundary before re-pairing; half-cell margin catches it.
    let mut world = seed_world();
    if let Some(cfg) = world.get_resource_mut::<PhysicsConfig>() {
        cfg.broadphase_cell_size = 16.0;
        cfg.substeps = 1;
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
    // The heavy shover squeezes the ball against the wall over many steps.
    // The soft solver tolerates a few px of transient overlap, but the ball
    // must never cross the wall centerline x=20 — past it, the narrow-phase
    // normal flips and ejects it out the far side.
    for step in 0..30 {
        physics_step(&mut world);
        let ball_pos = world.get::<Position>(ball).unwrap().0;
        assert!(
            ball_pos.x < 20.0,
            "ball slipped through wall at step {step}: {ball_pos:?}"
        );
    }
}

#[test]
fn speculative_contact_stops_extreme_bullet_in_one_substep() {
    // 4000 px/s at one 1/30 s substep travels ~133 px against an 8 px-thick
    // wall. The positive-gap contact (D-064) clamps arrival at touching and
    // the stored-approach restitution still reflects at e=0.5.
    let mut world = seed_world();
    if let Some(cfg) = world.get_resource_mut::<PhysicsConfig>() {
        cfg.substeps = 1;
    }
    if let Some(dt) = world.get_resource_mut::<DeltaTime>() {
        dt.dt = 1.0 / 30.0;
    }

    let ball = world.spawn();
    world.insert(ball, Position(Vec2::new(0.0, 0.0)));
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
        "ball tunneled through wall despite speculative contact: {:?}",
        pos.0
    );
    let vel = world.get::<Velocity>(ball).unwrap().0;
    assert!(
        vel.x < 0.0,
        "velocity should reflect off wall under speculative restitution: {vel:?}"
    );
}

/// Diagonal shover rig: a massive body moving +x hits the origin target
/// through a 45-degree contact normal, injecting ~(15k, 15k) px/s the pair
/// admission never saw (the shover's own path stays clear of the downrange
/// static). Returns the launched target entity.
fn spawn_diagonal_shover_rig(world: &mut World) -> Entity {
    if let Some(cfg) = world.get_resource_mut::<PhysicsConfig>() {
        cfg.substeps = 1;
    }

    let target = world.spawn();
    world.insert(target, Position(Vec2::new(0.0, 0.0)));
    world.insert(target, Velocity(Vec2::ZERO));
    world.insert(target, Collider::circle(4.0));
    world.insert(target, RigidBody::dynamic());

    // Center distance 9 at 45 degrees: 1 px gap, admitted speculatively.
    let shover = world.spawn();
    world.insert(shover, Position(Vec2::new(-6.364, -6.364)));
    world.insert(shover, Velocity(Vec2::new(30_000.0, 0.0)));
    world.insert(shover, Collider::circle(4.0));
    world.insert(shover, RigidBody::dynamic().with_mass(1_000.0));

    target
}

#[test]
fn sweep_net_catches_solver_injected_velocity_through_static_wall() {
    // The speculative pair admission sees pre-solve velocities; velocity
    // injected mid-substep by the solver sends the target through a slab it
    // was never paired with. The slab-sweep safety net must clamp it.
    let mut world = seed_world();
    let target = spawn_diagonal_shover_rig(&mut world);

    // Thin floor slab across the target's diagonal path (top face y=54),
    // outside the shover's horizontal travel band.
    let slab = world.spawn();
    world.insert(slab, Position(Vec2::new(250.0, 56.0)));
    world.insert(slab, Collider::aabb(Vec2::new(250.0, 2.0)));
    world.insert(slab, RigidBody::r#static());

    physics_step(&mut world);

    let pos = world.get::<Position>(target).unwrap();
    assert!(
        pos.0.y + 4.0 <= 54.0 + 0.5,
        "solver-injected velocity tunneled the static slab: {:?}",
        pos.0
    );
}

#[test]
fn sweep_net_covers_static_circles_via_bounding_square() {
    // Same solver-injected velocity spike, but the downrange static is a
    // circle: the safety net promotes it to its bounding square (D-064).
    let mut world = seed_world();
    let target = spawn_diagonal_shover_rig(&mut world);

    // Static circle centered on the diagonal path; bounding square [46,62]^2.
    let pillar = world.spawn();
    world.insert(pillar, Position(Vec2::new(54.0, 54.0)));
    world.insert(pillar, Collider::circle(8.0));
    world.insert(pillar, RigidBody::r#static());

    physics_step(&mut world);

    let pos = world.get::<Position>(target).unwrap();
    // Bounding-square near faces at 46; the target must stop at or before
    // them instead of passing through the pillar center.
    assert!(
        pos.0.x + 4.0 <= 46.0 + 0.5 || pos.0.y + 4.0 <= 46.0 + 0.5,
        "solver-injected velocity tunneled the static circle: {:?}",
        pos.0
    );
}

#[test]
fn speculative_gap_contact_emits_no_event_until_touch() {
    // Event gate (D-064): a positive-gap speculative contact enters the
    // solver but must not emit a CollisionEvent until actually penetrating.
    let mut world = seed_world();

    // 240 px/s = 1 px per default substep; start 3.5 px from touching so the
    // whole first frame stays separated while the last substeps admit the
    // gap contact.
    let ball = world.spawn();
    world.insert(ball, Position(Vec2::new(32.5, 0.0)));
    world.insert(ball, Velocity(Vec2::new(240.0, 0.0)));
    world.insert(ball, Collider::circle(4.0));
    world.insert(ball, RigidBody::dynamic());

    let wall = world.spawn();
    world.insert(wall, Position(Vec2::new(44.0, 0.0)));
    world.insert(wall, Collider::aabb(Vec2::new(4.0, 32.0)));
    world.insert(wall, RigidBody::r#static());

    physics_step(&mut world);
    {
        let events = world.get_resource::<EventQueue<CollisionEvent>>().unwrap();
        assert!(
            !events.iter_current().any(|e| e.a == ball),
            "speculative gap contact emitted an event before touch"
        );
    }
    world
        .get_resource_mut::<EventQueue<CollisionEvent>>()
        .unwrap()
        .flush();

    physics_step(&mut world);
    let events = world.get_resource::<EventQueue<CollisionEvent>>().unwrap();
    assert!(
        events
            .iter_current()
            .any(|e| e.a == ball && e.penetration > 0.0),
        "touching contact must emit with positive penetration"
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

    // Contacts are measured pre-solve, so the impact lands on step 2; run a
    // few steps so the multi-tile contact actually resolves.
    for _ in 0..3 {
        physics_step(&mut world);
        let vel = world.get::<Velocity>(player).unwrap().0;
        assert!(
            vel.y >= -1e-3,
            "zero-restitution body bounced upward off flat floor: {vel:?}"
        );
    }
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

    // Contacts are measured pre-solve, so the impact lands on step 2.
    for _ in 0..3 {
        physics_step(&mut world);
    }

    let vel = world.get::<Velocity>(ball).unwrap().0;
    // Bound catches old 2x-3x seam amplification.
    assert!(
        vel.y > -60.0,
        "ball impulse was doubled — rebounded too fast: {vel:?}"
    );
    // The restitution pass must still fire once: stored approach ~-50 px/s
    // at e=0.85 rebounds near -42 px/s.
    assert!(
        vel.y < -20.0,
        "restitution was lost across the tile seam: {vel:?}"
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

    // Floor top y=100; ball top <= 100 plus rest tolerance. The soft solver
    // rests at linear_slop plus a small load-dependent sink for the bottom
    // of a column (D-063); step 3's fixed substeps lift the hertz cap and
    // tighten this again.
    let floor_top = 100.0;
    for (i, &ball) in ball_entities.iter().enumerate() {
        let pos = world.get::<Position>(ball).unwrap().0;
        assert!(
            pos.y + RADIUS <= floor_top + 1.0,
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

#[test]
fn settled_bodies_calm_below_jitter_floor() {
    // Step 2 (D-063): warm-started accumulated impulses + soft bias + the
    // inelastic restitution threshold must collapse the old solver's jitter
    // floor (~97 px/s at pile scale) to near-zero rest speeds — the
    // prerequisite for step 4's sleep criterion.
    let mut world = seed_world();
    if let Some(cfg) = world.get_resource_mut::<PhysicsConfig>() {
        cfg.gravity = Vec2::new(0.0, 900.0);
        // Sleeping would zero velocities and mask the raw jitter floor this
        // test pins (D-063); measure with it disabled.
        cfg.sleep_threshold = 0.0;
    }

    let floor = world.spawn();
    world.insert(floor, Position(Vec2::new(0.0, 110.0)));
    world.insert(floor, Collider::aabb(Vec2::new(64.0, 8.0)));
    world.insert(floor, RigidBody::r#static());

    // Small column: floor top y=102, balls stacked with slim gaps.
    const RADIUS: f32 = 6.0;
    let mut balls = Vec::new();
    for i in 0..3 {
        let e = world.spawn();
        let y = 102.0 - RADIUS - (i as f32) * (RADIUS * 2.0 + 0.5);
        world.insert(e, Position(Vec2::new(0.0, y)));
        world.insert(e, Velocity(Vec2::ZERO));
        world.insert(e, Collider::circle(RADIUS));
        world.insert(e, RigidBody::dynamic().with_restitution(0.1));
        balls.push(e);
    }

    for _ in 0..180 {
        physics_step(&mut world);
        world
            .get_resource_mut::<EventQueue<CollisionEvent>>()
            .unwrap()
            .flush();
    }

    for (i, &ball) in balls.iter().enumerate() {
        let speed = world.get::<Velocity>(ball).unwrap().0.length();
        assert!(
            speed < 2.0,
            "ball {i} still jitters after settling: {speed} px/s"
        );
    }
}

#[test]
fn resting_contact_emits_until_asleep_and_resumes_on_wake() {
    // Event semantics (D-065, Box2D precedent): a resting contact keeps
    // emitting an event every step *while awake* (pre-solve contact set,
    // D-063 unchanged), goes silent once its island sleeps (sleeping bodies
    // never enter the narrow phase), and resumes emitting when woken.
    let mut world = seed_world();
    if let Some(cfg) = world.get_resource_mut::<PhysicsConfig>() {
        cfg.gravity = Vec2::new(0.0, 900.0);
    }

    let floor = world.spawn();
    world.insert(floor, Position(Vec2::new(0.0, 20.0)));
    world.insert(floor, Collider::aabb(Vec2::new(64.0, 8.0)));
    world.insert(floor, RigidBody::r#static());

    let ball = world.spawn();
    world.insert(ball, Position(Vec2::new(0.0, 9.0)));
    world.insert(ball, Velocity(Vec2::ZERO));
    world.insert(ball, Collider::circle(4.0));
    world.insert(ball, RigidBody::dynamic());

    // Land; rest penetration stays ~linear_slop, keeping the contact alive.
    for _ in 0..10 {
        physics_step(&mut world);
        world
            .get_resource_mut::<EventQueue<CollisionEvent>>()
            .unwrap()
            .flush();
    }

    // Awake resting contact: emits every step (still inside the 0.5 s
    // sleep window).
    for step in 0..5 {
        physics_step(&mut world);
        {
            let events = world.get_resource::<EventQueue<CollisionEvent>>().unwrap();
            assert!(
                events
                    .iter_current()
                    .any(|e| e.a == ball && e.penetration > 0.0),
                "awake resting contact stopped emitting events at step {step}"
            );
        }
        world
            .get_resource_mut::<EventQueue<CollisionEvent>>()
            .unwrap()
            .flush();
    }

    // Past time_to_sleep: the island sleeps and emission stops.
    for _ in 0..60 {
        physics_step(&mut world);
        world
            .get_resource_mut::<EventQueue<CollisionEvent>>()
            .unwrap()
            .flush();
    }
    assert!(
        world
            .get_resource::<PhysicsBuffers>()
            .unwrap()
            .is_sleeping(ball),
        "resting ball never fell asleep"
    );
    for step in 0..5 {
        physics_step(&mut world);
        {
            let events = world.get_resource::<EventQueue<CollisionEvent>>().unwrap();
            assert!(
                !events.iter_current().any(|e| e.a == ball),
                "sleeping resting contact emitted an event at step {step}"
            );
        }
        world
            .get_resource_mut::<EventQueue<CollisionEvent>>()
            .unwrap()
            .flush();
    }

    // Waking resumes emission on the next step.
    wake(&mut world, ball);
    physics_step(&mut world);
    let events = world.get_resource::<EventQueue<CollisionEvent>>().unwrap();
    assert!(
        events
            .iter_current()
            .any(|e| e.a == ball && e.penetration > 0.0),
        "woken resting contact did not resume emitting events"
    );
}

/// Spawn the shared sleep rig: static floor (top y=102) with a two-ball
/// column resting on it; settles and sleeps well within 150 steps.
fn spawn_sleeping_stack(world: &mut World) -> (Entity, Entity) {
    if let Some(cfg) = world.get_resource_mut::<PhysicsConfig>() {
        cfg.gravity = Vec2::new(0.0, 900.0);
    }

    let floor = world.spawn();
    world.insert(floor, Position(Vec2::new(0.0, 110.0)));
    world.insert(floor, Collider::aabb(Vec2::new(64.0, 8.0)));
    world.insert(floor, RigidBody::r#static());

    const RADIUS: f32 = 6.0;
    let mut balls = Vec::with_capacity(2);
    for i in 0..2 {
        let e = world.spawn();
        let y = 102.0 - RADIUS - (i as f32) * (RADIUS * 2.0 + 0.5);
        world.insert(e, Position(Vec2::new(0.0, y)));
        world.insert(e, Velocity(Vec2::ZERO));
        world.insert(e, Collider::circle(RADIUS));
        world.insert(e, RigidBody::dynamic().with_restitution(0.1));
        balls.push(e);
    }
    (balls[0], balls[1])
}

fn settle_to_sleep(world: &mut World, entities: &[Entity]) {
    for _ in 0..150 {
        physics_step(world);
        world
            .get_resource_mut::<EventQueue<CollisionEvent>>()
            .unwrap()
            .flush();
    }
    let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
    for (i, &e) in entities.iter().enumerate() {
        assert!(buffers.is_sleeping(e), "body {i} never fell asleep");
    }
}

#[test]
fn settled_island_sleeps_and_freezes() {
    // D-065: a settled island sleeps (velocities zeroed) and its members are
    // bit-frozen — no integration, narrow phase, or writeback touches them.
    let mut world = seed_world();
    let (bottom, top) = spawn_sleeping_stack(&mut world);
    settle_to_sleep(&mut world, &[bottom, top]);

    for &e in &[bottom, top] {
        assert_eq!(
            world.get::<Velocity>(e).unwrap().0,
            Vec2::ZERO,
            "sleeping body kept a nonzero velocity"
        );
    }
    let frozen: Vec<Vec2> = [bottom, top]
        .iter()
        .map(|&e| world.get::<Position>(e).unwrap().0)
        .collect();
    for _ in 0..30 {
        physics_step(&mut world);
    }
    for (i, &e) in [bottom, top].iter().enumerate() {
        assert_eq!(
            world.get::<Position>(e).unwrap().0,
            frozen[i],
            "sleeping body {i} moved"
        );
    }
}

#[test]
fn bullet_wakes_sleeping_body_through_speculative_gap() {
    // D-065 wake path: a fast body approaching a sleeper is admitted as a
    // speculative gap contact and wakes it before touch resolves — the
    // sleeper must not be tunneled through while frozen.
    let mut world = seed_world();

    let target = world.spawn();
    world.insert(target, Position(Vec2::new(200.0, 0.0)));
    world.insert(target, Velocity(Vec2::ZERO));
    world.insert(target, Collider::circle(4.0));
    world.insert(target, RigidBody::dynamic());

    for _ in 0..40 {
        physics_step(&mut world);
    }
    assert!(
        world
            .get_resource::<PhysicsBuffers>()
            .unwrap()
            .is_sleeping(target),
        "idle zero-gravity body never slept"
    );

    let bullet = world.spawn();
    world.insert(bullet, Position(Vec2::new(0.0, 0.0)));
    world.insert(bullet, Velocity(Vec2::new(3_000.0, 0.0)));
    world.insert(bullet, Collider::circle(4.0));
    world.insert(bullet, RigidBody::dynamic());

    for _ in 0..6 {
        physics_step(&mut world);
    }

    assert!(
        !world
            .get_resource::<PhysicsBuffers>()
            .unwrap()
            .is_sleeping(target),
        "bullet impact did not wake the sleeping target"
    );
    let target_vel = world.get::<Velocity>(target).unwrap().0;
    assert!(
        target_vel.x > 0.0,
        "no momentum transferred to woken target: {target_vel:?}"
    );
    let bullet_x = world.get::<Position>(bullet).unwrap().0.x;
    let target_x = world.get::<Position>(target).unwrap().0.x;
    assert!(
        bullet_x < target_x,
        "bullet passed through the sleeping target: bullet {bullet_x}, target {target_x}"
    );
}

#[test]
fn external_velocity_write_wakes_island() {
    let mut world = seed_world();
    let (bottom, top) = spawn_sleeping_stack(&mut world);
    settle_to_sleep(&mut world, &[bottom, top]);

    world.get_mut::<Velocity>(top).unwrap().0 = Vec2::new(0.0, -300.0);
    physics_step(&mut world);

    let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
    assert!(!buffers.is_sleeping(top), "written body stayed asleep");
    assert!(
        !buffers.is_sleeping(bottom),
        "island member of written body stayed asleep"
    );
    assert!(
        world.get::<Position>(top).unwrap().0.y < 102.0 - 6.0 - 6.5,
        "woken body did not integrate its written velocity"
    );
}

#[test]
fn external_position_write_wakes_island() {
    let mut world = seed_world();
    let (bottom, top) = spawn_sleeping_stack(&mut world);
    settle_to_sleep(&mut world, &[bottom, top]);

    // Teleport the top ball into free air above the floor.
    world.get_mut::<Position>(top).unwrap().0 = Vec2::new(200.0, 0.0);
    physics_step(&mut world);

    let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
    assert!(!buffers.is_sleeping(top), "teleported body stayed asleep");
    assert!(
        !buffers.is_sleeping(bottom),
        "island member of teleported body stayed asleep"
    );
    for _ in 0..5 {
        physics_step(&mut world);
    }
    assert!(
        world.get::<Velocity>(top).unwrap().0.y > 0.0,
        "teleported body is not falling under gravity"
    );
}

#[test]
fn despawn_in_sleeping_island_wakes_the_rest() {
    let mut world = seed_world();
    let (bottom, top) = spawn_sleeping_stack(&mut world);
    settle_to_sleep(&mut world, &[bottom, top]);

    let top_y = world.get::<Position>(top).unwrap().0.y;
    world.despawn(bottom);
    for _ in 0..20 {
        physics_step(&mut world);
    }

    assert!(
        !world
            .get_resource::<PhysicsBuffers>()
            .unwrap()
            .is_sleeping(top),
        "island survivor stayed asleep after member despawn"
    );
    assert!(
        world.get::<Position>(top).unwrap().0.y > top_y + 5.0,
        "unsupported body did not fall after its support despawned"
    );
}

#[test]
fn wake_api_wakes_whole_island_and_it_can_resleep() {
    let mut world = seed_world();
    let (bottom, top) = spawn_sleeping_stack(&mut world);
    settle_to_sleep(&mut world, &[bottom, top]);

    wake(&mut world, top);
    {
        let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
        assert!(!buffers.is_sleeping(top), "wake() left the target asleep");
        assert!(
            !buffers.is_sleeping(bottom),
            "wake() did not wake the rest of the island"
        );
    }

    // Undisturbed, the island sleeps again after time_to_sleep.
    for _ in 0..40 {
        physics_step(&mut world);
        world
            .get_resource_mut::<EventQueue<CollisionEvent>>()
            .unwrap()
            .flush();
    }
    let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
    assert!(
        buffers.is_sleeping(top) && buffers.is_sleeping(bottom),
        "woken island never re-slept"
    );
}

#[test]
fn late_sleeper_adopts_supporting_island_for_despawn_wake() {
    // D-065 tag adoption: a body that settles on an already sleeping island
    // joins that island's tag, so an island wake (despawn deep below) also
    // lifts it — otherwise it would float frozen in the air.
    let mut world = seed_world();
    if let Some(cfg) = world.get_resource_mut::<PhysicsConfig>() {
        cfg.gravity = Vec2::new(0.0, 900.0);
    }

    let floor = world.spawn();
    world.insert(floor, Position(Vec2::new(0.0, 110.0)));
    world.insert(floor, Collider::aabb(Vec2::new(64.0, 8.0)));
    world.insert(floor, RigidBody::r#static());

    const RADIUS: f32 = 6.0;
    let base = world.spawn();
    world.insert(base, Position(Vec2::new(0.0, 102.0 - RADIUS)));
    world.insert(base, Velocity(Vec2::ZERO));
    world.insert(base, Collider::circle(RADIUS));
    world.insert(base, RigidBody::dynamic());
    for _ in 0..60 {
        physics_step(&mut world);
    }
    assert!(
        world
            .get_resource::<PhysicsBuffers>()
            .unwrap()
            .is_sleeping(base),
        "base ball never slept"
    );

    // Lower a second ball gently onto the sleeper: the approach speed stays
    // under the wake tolerance, so the base stays asleep while the newcomer
    // settles on it and sleeps as a later island.
    let base_top = world.get::<Position>(base).unwrap().0.y - RADIUS;
    let rider = world.spawn();
    world.insert(rider, Position(Vec2::new(0.0, base_top - RADIUS - 0.3)));
    world.insert(rider, Velocity(Vec2::ZERO));
    world.insert(rider, Collider::circle(RADIUS));
    world.insert(rider, RigidBody::dynamic());
    for _ in 0..60 {
        physics_step(&mut world);
    }
    {
        let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
        assert!(buffers.is_sleeping(base), "gentle rider woke the base");
        assert!(buffers.is_sleeping(rider), "rider never slept");
    }

    let rider_y = world.get::<Position>(rider).unwrap().0.y;
    world.despawn(base);
    for _ in 0..20 {
        physics_step(&mut world);
    }
    assert!(
        world.get::<Position>(rider).unwrap().0.y > rider_y + 5.0,
        "rider stayed frozen in the air after its support despawned"
    );
}

#[test]
fn contact_wake_stays_local_to_the_disturbance() {
    // D-065 locality: waking spreads through contacts only while motion
    // exceeds the threshold, so a gentle poke on one end of a sleeping row
    // must not wake the far end (no whole-island wake on contact).
    let mut world = seed_world();
    if let Some(cfg) = world.get_resource_mut::<PhysicsConfig>() {
        cfg.gravity = Vec2::new(0.0, 900.0);
    }

    let floor = world.spawn();
    world.insert(floor, Position(Vec2::new(0.0, 110.0)));
    world.insert(floor, Collider::aabb(Vec2::new(256.0, 8.0)));
    world.insert(floor, RigidBody::r#static());

    const RADIUS: f32 = 6.0;
    const COUNT: usize = 16;
    let mut row = Vec::with_capacity(COUNT);
    for i in 0..COUNT {
        let e = world.spawn();
        world.insert(
            e,
            Position(Vec2::new(i as f32 * (RADIUS * 2.0), 102.0 - RADIUS)),
        );
        world.insert(e, Velocity(Vec2::ZERO));
        world.insert(e, Collider::circle(RADIUS));
        world.insert(e, RigidBody::dynamic());
        row.push(e);
    }
    for _ in 0..60 {
        physics_step(&mut world);
    }
    {
        let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
        for (i, &e) in row.iter().enumerate() {
            assert!(buffers.is_sleeping(e), "row ball {i} never slept");
        }
    }

    // Gentle poke: drop a ball a short distance onto the leftmost member.
    let poke = world.spawn();
    world.insert(poke, Position(Vec2::new(0.0, 102.0 - RADIUS * 3.0 - 8.0)));
    world.insert(poke, Velocity(Vec2::ZERO));
    world.insert(poke, Collider::circle(RADIUS));
    world.insert(poke, RigidBody::dynamic());
    for _ in 0..5 {
        physics_step(&mut world);
    }

    let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
    for (i, &e) in row.iter().enumerate().skip(COUNT / 2) {
        assert!(
            buffers.is_sleeping(e),
            "poke on ball 0 woke distant ball {i}"
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

#[test]
fn diagnostic_counts_track_the_last_step() {
    let mut world = seed_world();
    assert_eq!(PhysicsBuffers::default().proxy_count(), 0);
    let (bottom, top) = spawn_sleeping_stack(&mut world);

    physics_step(&mut world);
    let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
    assert_eq!(buffers.proxy_count(), 3, "floor plus two balls");
    assert_eq!(
        buffers.dynamic_count(),
        2,
        "the static floor is not dynamic"
    );
    assert_eq!(buffers.sleeping_count(), 0);
    assert!(buffers.pair_count() >= 2, "ball-ball and ball-floor pair");
    assert!(buffers.contact_count() >= 1);

    settle_to_sleep(&mut world, &[bottom, top]);
    physics_step(&mut world);
    let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
    assert_eq!(buffers.dynamic_count(), 2);
    assert_eq!(buffers.sleeping_count(), 2);
    assert_eq!(buffers.pair_count(), 0, "sleeping bodies never initiate");
}

/// Compare the persistent list with fresh per-substep pair finding and an
/// exhaustive oracle, before any contact wakes mutate the snapshot. The
/// oracle prevents a bug shared by both broadphases from hiding a missed pair.
pub(super) fn assert_pair_contacts(config: &PhysicsConfig, sub_dt: f32, buffers: &PhysicsBuffers) {
    use std::collections::BTreeSet;

    let proxies = &buffers.proxies;
    let contact_key = |a: usize, b: usize| {
        let margin = (proxies[a].velocity - proxies[b].velocity).length() * sub_dt
            + 4.0 * config.linear_slop;
        narrow_phase(&proxies[a], &proxies[b], margin)
            .map(|_| pair_key(proxies[a].key, proxies[b].key))
    };
    let mut persistent = BTreeSet::new();
    for &(a, b) in &buffers.pairs {
        if let Some(key) = contact_key(a as usize, b as usize) {
            assert!(persistent.insert(key), "duplicate persistent contact");
        }
    }

    let mut fresh_grid = SpatialGrid::new(config.broadphase_cell_size);
    let bounds: Vec<_> = proxies
        .iter()
        .enumerate()
        .map(|(i, proxy)| {
            let mut aabb = proxy.world_aabb();
            aabb.half_extents +=
                Vec2::splat(proxy.velocity.length() * sub_dt + 2.0 * config.linear_slop);
            fresh_grid.insert(i as u32, &aabb);
            aabb
        })
        .collect();
    let mut fresh = BTreeSet::new();
    let mut exhaustive = BTreeSet::new();
    for (a, proxy) in proxies.iter().enumerate() {
        if !proxy.is_dynamic || proxy.sleeping {
            continue;
        }
        let eligible =
            |b: usize| a != b && (!proxies[b].is_dynamic || proxies[b].sleeping || b > a);
        fresh_grid.for_each_in(&bounds[a], Some(a as u32), |b| {
            let b = b as usize;
            if eligible(b)
                && bounds[a].overlaps(&bounds[b])
                && let Some(key) = contact_key(a, b)
            {
                assert!(fresh.insert(key), "duplicate fresh contact");
            }
        });
        for b in 0..proxies.len() {
            if eligible(b)
                && let Some(key) = contact_key(a, b)
            {
                exhaustive.insert(key);
            }
        }
    }
    assert_eq!(fresh, exhaustive, "fresh pair finding missed a contact");
    assert_eq!(
        persistent, exhaustive,
        "persistent pair list missed a contact"
    );
}

#[test]
fn persistent_pairs_match_fresh_contacts_on_randomized_piles_bullets_and_wakes() {
    use crate::Pcg32;

    let mut checked = 0;
    for seed in 0..8 {
        let mut rng = Pcg32::seeded(0xD075 + seed);
        let mut world = seed_world();
        let (bottom, top) = spawn_sleeping_stack(&mut world);
        settle_to_sleep(&mut world, &[bottom, top]);
        let buffers = world.get_resource_mut::<PhysicsBuffers>().unwrap();
        buffers.check_pair_contacts = true;
        buffers.reference_impulses = Some(buffers.impulses.clone());
        // Cover differing substep counts and cell boundaries, mixed shape
        // pairs, sleepers and fast bullets in the same evolving snapshot.
        let config = world.get_resource_mut::<PhysicsConfig>().unwrap();
        config.substeps = [1, 2, 4, 8][seed as usize % 4];
        config.broadphase_cell_size = [8.0, 16.0, 32.0][seed as usize % 3];
        for i in 0..36 {
            let e = world.spawn();
            world.insert(
                e,
                Position(Vec2::new(
                    (i % 6) as f32 * 11.0 - 28.0 + rng.next_range(-1.0, 1.0),
                    65.0 - (i / 6) as f32 * 11.0 + rng.next_range(-1.0, 1.0),
                )),
            );
            world.insert(
                e,
                Velocity(rng.next_unit_vec2() * rng.next_range(0.0, 80.0)),
            );
            world.insert(
                e,
                RigidBody::dynamic().with_restitution(rng.next_range(0.0, 0.5)),
            );
            world.insert(
                e,
                if i % 3 == 0 {
                    Collider::aabb(Vec2::splat(5.0))
                } else {
                    Collider::circle(5.0)
                },
            );
        }
        let bullet = world.spawn();
        world.insert(bullet, Position(Vec2::new(-90.0, 95.0)));
        world.insert(
            bullet,
            Velocity(Vec2::new(rng.next_range(2400.0, 15000.0), 0.0)),
        );
        world.insert(bullet, Collider::circle(4.0));
        world.insert(bullet, RigidBody::dynamic().with_restitution(0.8));
        for frame in 0..40 {
            match frame {
                8 => wake(&mut world, top),
                16 => world.get_mut::<Velocity>(bottom).unwrap().0 = Vec2::new(0.0, -180.0),
                24 => {
                    world.despawn(top);
                }
                _ => {}
            }
            physics_step(&mut world);
            world
                .get_resource_mut::<EventQueue<CollisionEvent>>()
                .unwrap()
                .flush();
        }
        let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
        assert!(
            !buffers.is_sleeping(bottom),
            "wake cases never disturbed the sleeper"
        );
        checked += buffers.checked_substeps;
    }
    assert_eq!(
        checked, 1200,
        "every substep must run the equivalence oracle"
    );
}

#[test]
fn contact_woken_body_rebuilds_pairs_over_static_floor_in_waking_frame() {
    let mut world = seed_world();
    let (bottom, top) = spawn_sleeping_stack(&mut world);
    settle_to_sleep(&mut world, &[bottom, top]);
    // Hit the bottom body horizontally, below the top body: the first
    // build must omit sleeper-floor, then include it immediately after wake.
    let y = world.get::<Position>(bottom).unwrap().0.y;
    let bullet = world.spawn();
    world.insert(bullet, Position(Vec2::new(-20.0, y)));
    world.insert(bullet, Velocity(Vec2::new(2400.0, 0.0)));
    world.insert(bullet, Collider::circle(4.0));
    world.insert(bullet, RigidBody::dynamic());
    let config = *world.get_resource::<PhysicsConfig>().unwrap();
    let dt = 1.0 / 60.0;
    let sub_dt = dt / config.substeps as f32;
    let mut buffers = world.remove_resource::<PhysicsBuffers>().unwrap();
    gather_proxies(&world, &mut buffers.proxies);
    sleep_frame_start(&mut buffers);
    buffers.island_parent = (0..buffers.proxies.len() as u32).collect();
    buffers.events.clear();
    buffers.pairs_invalidated = true;
    buffers.check_pair_contacts = true;
    let bottom_idx = buffers
        .proxies
        .iter()
        .position(|p| p.entity == Some(bottom))
        .unwrap();
    let floor_idx = buffers.proxies.iter().position(|p| !p.is_dynamic).unwrap();
    let floor_pair = |pairs: &[(u32, u32)]| {
        pairs
            .iter()
            .any(|&(a, b)| a as usize == bottom_idx && b as usize == floor_idx)
    };
    build_pairs(&config, dt, &mut buffers);
    assert!(!floor_pair(&buffers.pairs), "sleepers must not initiate");
    substep(&config, sub_dt, dt, &mut buffers, true);
    assert!(
        !buffers.proxies[bottom_idx].sleeping,
        "impact must wake this substep"
    );
    assert!(
        buffers.pairs_invalidated,
        "wake must invalidate even if travel fits"
    );
    let builds = buffers.pair_builds;
    for step in 1..config.substeps {
        substep(
            &config,
            sub_dt,
            dt - step as f32 * sub_dt,
            &mut buffers,
            true,
        );
        assert!(
            floor_pair(&buffers.pairs),
            "woken body missing its static floor"
        );
        assert!(
            buffers.proxies[bottom_idx].center.y + 6.0 < 103.0,
            "woken body fell through its floor"
        );
    }
    assert!(buffers.pair_builds > builds);
    assert!(
        buffers
            .contacts
            .iter()
            .any(|c| c.a as usize == bottom_idx && c.b as usize == floor_idx)
    );
}

#[test]
fn fast_body_trips_pair_budget_and_rebuilds_before_narrow_phase() {
    let mut world = seed_world();
    let bullet = world.spawn();
    world.insert(bullet, Position(Vec2::ZERO));
    world.insert(bullet, Velocity(Vec2::new(240.0, 0.0)));
    world.insert(bullet, Collider::circle(4.0));
    world.insert(bullet, RigidBody::dynamic());
    let wall = world.spawn();
    world.insert(wall, Position(Vec2::new(48.0, 0.0)));
    world.insert(wall, Collider::aabb(Vec2::new(2.0, 32.0)));
    world.insert(wall, RigidBody::r#static());
    let config = *world.get_resource::<PhysicsConfig>().unwrap();
    let dt = 1.0 / 60.0;
    let sub_dt = dt / config.substeps as f32;
    let mut buffers = PhysicsBuffers::default();
    gather_proxies(&world, &mut buffers.proxies);
    buffers.island_parent = (0..buffers.proxies.len() as u32).collect();
    buffers.check_pair_contacts = true;
    build_pairs(&config, dt, &mut buffers);
    assert!(
        buffers.pairs.is_empty(),
        "wall should start outside the budget"
    );
    let idx = buffers
        .proxies
        .iter()
        .position(|p| p.entity == Some(bullet))
        .unwrap();
    substep(&config, sub_dt, dt, &mut buffers, false);
    assert_eq!(buffers.pair_builds, 1, "unspent budget should reuse pairs");
    assert!(buffers.pair_budgets[idx].travel > 0.0);
    // Model an impulse spike after the first build. The next admission must
    // see the wall even though it wasn't a candidate in the old list.
    buffers.proxies[idx].velocity = Vec2::new(15000.0, 0.0);
    assert!(pair_budget_exhausted(&config, sub_dt, &buffers));
    substep(&config, sub_dt, dt - sub_dt, &mut buffers, false);
    assert_eq!(buffers.pair_builds, 2);
    assert!(buffers.contacts.iter().any(|c| c.a as usize == idx));
    assert!(
        buffers.proxies[idx].center.x + 4.0 <= 46.5,
        "budget trip tunneled wall"
    );
    assert!(
        (buffers.pair_budgets[idx].radius - (15000.0 * (dt - sub_dt) + 2.0 * config.linear_slop))
            .abs()
            < 1e-4,
        "rebuild must use the remaining frame time"
    );
    // Accumulated travel is independently sufficient to invalidate even
    // after velocity drops to zero (e.g. a solver/sweep clamp).
    buffers.proxies[idx].velocity = Vec2::ZERO;
    buffers.pair_budgets[idx].travel = buffers.pair_budgets[idx].radius;
    assert!(pair_budget_exhausted(&config, sub_dt, &buffers));
}

#[test]
fn warm_start_forgets_disappearing_contacts_and_syncs_empty_final_substep() {
    let mut world = seed_world();
    let body = world.spawn();
    world.insert(body, Position(Vec2::ZERO));
    world.insert(body, Velocity(Vec2::new(100.0, 0.0)));
    world.insert(body, Collider::circle(5.0));
    world.insert(body, RigidBody::dynamic());
    let wall = world.spawn();
    world.insert(wall, Position(Vec2::new(8.0, 0.0)));
    world.insert(wall, Collider::circle(5.0));
    let config = PhysicsConfig {
        gravity: Vec2::ZERO,
        ..PhysicsConfig::default()
    };
    let mut buffers = PhysicsBuffers::default();
    gather_proxies(&world, &mut buffers.proxies);
    buffers.reference_impulses = Some(ImpulseMap::default());
    let body_index = buffers
        .proxies
        .iter()
        .position(|p| p.entity == Some(body))
        .unwrap();
    let key = pair_key(entity_key(body), entity_key(wall));
    // A generous real travel budget keeps the candidate present while the
    // exact contact disappears and returns. The reference map checks the
    // warm-start value before every solve, not only the final result.
    build_pairs(&config, 1.0, &mut buffers);
    substep(&config, 1.0 / 240.0, 1.0, &mut buffers, false);
    assert!(buffers.contacts.iter().any(|c| c.impulse > 0.0));
    let builds = buffers.pair_builds;
    for (x, rebuild) in [
        (-30.0, false),
        (0.0, false),
        (-30.0, true),
        (0.0, true),
        (-30.0, false),
    ] {
        buffers.proxies[body_index].center = Vec2::new(x, 0.0);
        buffers.proxies[body_index].velocity = Vec2::ZERO;
        if rebuild {
            buffers.pairs_invalidated = true;
        }
        substep(&config, 1.0 / 240.0, 1.0, &mut buffers, false);
        if x < 0.0 {
            assert!(buffers.contacts.is_empty());
        }
    }
    assert!(
        buffers.pair_builds > builds,
        "exercise reordered pair-cache boundary"
    );
    sync_impulses(&mut buffers);
    assert_eq!(buffers.impulses.get(key), 0.0);
    assert!(buffers.impulses.keys.iter().all(|key| *key == EMPTY_PAIR));
    assert!(!buffers.impulses_dirty);
}
