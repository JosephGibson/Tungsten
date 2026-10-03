use super::pairs::{
    EMPTY_PAIR, PAIR_AWAKE, PAIR_MARGIN_SLOPS, build_pairs, collect_tripped, gravity_allowance,
};
use super::*;
use crate::assets::{LayerKind, TilemapInstance};
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
fn sweep_net_restages_statics_when_the_proxy_set_changes() {
    // The sweep grid is staged per frame (D-080). After a static despawns and
    // two spawn, proxy indices shift: the old slab must stop clamping and the
    // new one must clamp.
    let mut world = seed_world();
    world.get_resource_mut::<PhysicsConfig>().unwrap().substeps = 1;
    // Three stray bullets sweep first every frame, so the statics-only grid
    // is staged before the target's query.
    for i in 0..3 {
        let bullet = world.spawn();
        world.insert(bullet, Position(Vec2::new(i as f32 * 100.0, -2_000.0)));
        world.insert(bullet, Velocity(Vec2::new(30_000.0, 0.0)));
        world.insert(bullet, Collider::circle(4.0));
        world.insert(bullet, RigidBody::dynamic());
    }
    let target = world.spawn();
    world.insert(target, Position(Vec2::ZERO));
    world.insert(target, Velocity(Vec2::ZERO));
    world.insert(target, Collider::circle(4.0));
    world.insert(target, RigidBody::dynamic());
    let shover = world.spawn();
    world.insert(shover, Position(Vec2::ZERO));
    world.insert(shover, Velocity(Vec2::ZERO));
    world.insert(shover, Collider::circle(4.0));
    world.insert(shover, RigidBody::dynamic().with_mass(1_000.0));
    // Same geometry as `spawn_diagonal_shover_rig`, re-armed every frame.
    let arm = |world: &mut World| {
        world.get_mut::<Position>(target).unwrap().0 = Vec2::ZERO;
        world.get_mut::<Velocity>(target).unwrap().0 = Vec2::ZERO;
        world.get_mut::<Position>(shover).unwrap().0 = Vec2::new(-6.364, -6.364);
        world.get_mut::<Velocity>(shover).unwrap().0 = Vec2::new(30_000.0, 0.0);
    };
    let spawn_static = |world: &mut World, center: Vec2, half: Vec2| {
        let e = world.spawn();
        world.insert(e, Position(center));
        world.insert(e, Collider::aabb(half));
        world.insert(e, RigidBody::r#static());
        e
    };

    let staged = |world: &World| {
        let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
        buffers.static_grid.staged
    };

    let near = spawn_static(&mut world, Vec2::new(250.0, 56.0), Vec2::new(250.0, 2.0));
    arm(&mut world);
    physics_step(&mut world);
    assert!(staged(&world), "frame 1 never used the statics-only grid");
    let y = world.get::<Position>(target).unwrap().0.y;
    assert!(y + 4.0 <= 54.5, "near slab did not clamp: y={y}");

    world.despawn(near);
    spawn_static(&mut world, Vec2::splat(-5_000.0), Vec2::splat(4.0));
    spawn_static(&mut world, Vec2::new(250.0, 156.0), Vec2::new(250.0, 2.0));
    arm(&mut world);
    physics_step(&mut world);
    assert!(staged(&world), "frame 2 never used the statics-only grid");
    let y = world.get::<Position>(target).unwrap().0.y;
    assert!(y + 4.0 > 60.0, "despawned slab still clamps: y={y}");
    assert!(y + 4.0 <= 154.5, "new slab did not clamp: y={y}");
}

#[test]
fn sweep_rents_the_pair_grid_until_queries_reach_the_static_count() {
    // D-080's rent-or-buy rule: with three statics, a frame's first three
    // sweep queries go to the pair grid and the fourth stages the
    // statics-only grid. A frame with fewer queries never stages it.
    let mut world = seed_world();
    world.get_resource_mut::<PhysicsConfig>().unwrap().substeps = 1;
    for i in 0..3 {
        let wall = world.spawn();
        world.insert(wall, Position(Vec2::new(-1_000.0, i as f32 * 100.0)));
        world.insert(wall, Collider::aabb(Vec2::splat(4.0)));
        world.insert(wall, RigidBody::r#static());
    }
    let spawn_bullet = |world: &mut World, y: f32| {
        let bullet = world.spawn();
        world.insert(bullet, Position(Vec2::new(0.0, y)));
        world.insert(bullet, Velocity(Vec2::new(30_000.0, 0.0)));
        world.insert(bullet, Collider::circle(4.0));
        world.insert(bullet, RigidBody::dynamic());
    };
    let sweep_state = |world: &World| {
        let sweep = &world.get_resource::<PhysicsBuffers>().unwrap().static_grid;
        (sweep.statics, sweep.pair_queries, sweep.staged)
    };

    spawn_bullet(&mut world, 0.0);
    spawn_bullet(&mut world, 100.0);
    physics_step(&mut world);
    assert_eq!(sweep_state(&world), (3, 2, false));

    spawn_bullet(&mut world, 200.0);
    spawn_bullet(&mut world, 300.0);
    physics_step(&mut world);
    assert_eq!(sweep_state(&world), (3, 3, true));
}

#[test]
fn sweep_grid_returns_the_pair_grids_statics_in_order() {
    // D-080: for any query the statics-only grid returns the static
    // candidates the pair grid returns, in the same order, so the sweep's
    // first hit cannot change. Covers tiles, body-less colliders, sleepers
    // (staged in the pair grid, never a sweep target) and three cell sizes.
    use crate::Pcg32;

    for (seed, cell) in [8.0, 16.0, 32.0].into_iter().enumerate() {
        let mut rng = Pcg32::seeded(0xD080 + seed as u64);
        let mut world = seed_world();
        world
            .get_resource_mut::<PhysicsConfig>()
            .unwrap()
            .broadphase_cell_size = cell;
        world.get_resource_mut::<TilemapRegistry>().unwrap().insert(
            "tiles".into(),
            TilemapData {
                tile_width: 16,
                tile_height: 16,
                width: 12,
                height: 2,
                tileset: vec!["solid".into()],
                layers: vec![TilemapLayer {
                    name: "collision".into(),
                    kind: LayerKind::Collision,
                    tiles: (0..24).map(|i| if i % 5 == 0 { -1 } else { 0 }).collect(),
                }],
            },
        );
        let map = world.spawn();
        world.insert(map, TilemapInstance::new("tiles", Vec2::new(-96.0, 40.0)));
        for i in 0..60 {
            let e = world.spawn();
            world.insert(
                e,
                Position(Vec2::new(
                    rng.next_range(-120.0, 120.0),
                    rng.next_range(-80.0, 80.0),
                )),
            );
            world.insert(
                e,
                if i % 2 == 0 {
                    Collider::aabb(Vec2::new(
                        rng.next_range(1.0, 30.0),
                        rng.next_range(1.0, 30.0),
                    ))
                } else {
                    Collider::circle(rng.next_range(1.0, 20.0))
                },
            );
            match i % 3 {
                0 => world.insert(e, RigidBody::r#static()),
                1 => {
                    world.insert(
                        e,
                        Velocity(rng.next_unit_vec2() * rng.next_range(0.0, 400.0)),
                    );
                    world.insert(e, RigidBody::dynamic());
                }
                _ => {}
            }
        }
        let config = *world.get_resource::<PhysicsConfig>().unwrap();
        let mut buffers = PhysicsBuffers::default();
        gather_proxies(&world, &mut buffers.proxies);
        for proxy in buffers.proxies.iter_mut().step_by(4) {
            proxy.sleeping = proxy.is_dynamic;
        }
        build_pairs(&config, 1.0 / 240.0, 1.0 / 60.0, &mut buffers);
        buffers.static_grid.stage(&config, &buffers.proxies);

        let mut from_pair_grid = Vec::new();
        let mut from_static_grid = Vec::new();
        let mut returned = 0;
        for _ in 0..200 {
            let probe = Aabb::new(
                Vec2::new(rng.next_range(-140.0, 140.0), rng.next_range(-100.0, 100.0)),
                Vec2::new(rng.next_range(0.0, 40.0), rng.next_range(0.0, 40.0)),
            );
            buffers.grid.query(&probe, None, &mut from_pair_grid);
            from_pair_grid.retain(|&id| !buffers.proxies[id as usize].is_dynamic);
            buffers
                .static_grid
                .grid
                .query(&probe, None, &mut from_static_grid);
            assert_eq!(from_static_grid, from_pair_grid, "cell {cell}, {probe:?}");
            returned += from_static_grid.len();
        }
        assert!(returned > 400, "probes met too few statics: {returned}");
    }
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
    // D-063: warm-started accumulated impulses + soft bias + the inelastic
    // restitution threshold must collapse the old solver's jitter floor
    // (~97 px/s at pile scale) to near-zero rest speeds — the prerequisite
    // for the sleep criterion (D-065).
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
fn sleeper_that_stops_being_a_dynamic_body_wakes_its_island() {
    // D-065 wake path 3 without a despawn: the entity and its key stay, but
    // it no longer has a sleep entry, so the rest of its island must wake.
    let mut world = seed_world();
    let (bottom, top) = spawn_sleeping_stack(&mut world);
    settle_to_sleep(&mut world, &[bottom, top]);

    world.insert(bottom, RigidBody::r#static());
    physics_step(&mut world);

    let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
    assert!(!buffers.is_sleeping(bottom), "a static body cannot sleep");
    assert!(!buffers.is_sleeping(top), "island survivor stayed asleep");
}

#[test]
fn sleep_table_is_rebuilt_only_when_the_body_sequence_changes() {
    // D-082: with the same bodies in the same order the entries stay at
    // their proxies' indices. A spawn or despawn shifts the sequence, and
    // the rebuild carries sleepers over by entity key.
    let mut world = seed_world();
    let (bottom, top) = spawn_sleeping_stack(&mut world);
    settle_to_sleep(&mut world, &[bottom, top]);
    let rebuilds = |world: &World| {
        world
            .get_resource::<PhysicsBuffers>()
            .unwrap()
            .sleep
            .rebuilds
    };
    let index_of = |world: &World, entity: Entity| {
        let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
        buffers
            .proxies
            .iter()
            .position(|proxy| proxy.entity == Some(entity))
    };
    assert_eq!(
        rebuilds(&world),
        1,
        "only the first frame lines the table up"
    );
    let bottom_index = index_of(&world, bottom);

    // A body-less collider gathers before every body and shifts the sleepers'
    // indices; a new dynamic body gathers after them.
    let pillar = world.spawn();
    world.insert(pillar, Position(Vec2::new(500.0, 0.0)));
    world.insert(pillar, Collider::circle(4.0));
    let drifter = world.spawn();
    world.insert(drifter, Position(Vec2::new(-500.0, 0.0)));
    world.insert(drifter, Velocity(Vec2::new(40.0, 0.0)));
    world.insert(drifter, Collider::circle(4.0));
    world.insert(drifter, RigidBody::dynamic());
    for _ in 0..3 {
        physics_step(&mut world);
    }
    assert_eq!(rebuilds(&world), 2);
    assert_ne!(
        index_of(&world, bottom),
        bottom_index,
        "sleeper kept its index"
    );
    let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
    assert!(buffers.is_sleeping(bottom) && buffers.is_sleeping(top));
    assert!(!buffers.is_sleeping(drifter) && !buffers.is_sleeping(pillar));
    assert_eq!(buffers.sleeping_count(), 2);
    assert_eq!(buffers.sleep.entries.len(), buffers.proxy_count());

    // `wake` still finds a body after its index moved.
    wake(&mut world, top);
    let buffers = world.get_resource::<PhysicsBuffers>().unwrap();
    assert!(!buffers.is_sleeping(bottom) && !buffers.is_sleeping(top));

    world.despawn(pillar);
    physics_step(&mut world);
    assert_eq!(rebuilds(&world), 3);
    physics_step(&mut world);
    assert_eq!(rebuilds(&world), 3);
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
    let mut repairs = 0;
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
        repairs += buffers.pair_repairs;
    }
    assert_eq!(
        checked, 1200,
        "every substep must run the equivalence oracle"
    );
    assert!(repairs > 0, "the oracle never checked a repaired list");
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
    buffers.static_grid.begin_frame();
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
    build_pairs(&config, sub_dt, dt, &mut buffers);
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
fn lone_fast_body_trips_its_budget_and_rebuilds_before_narrow_phase() {
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
    build_pairs(&config, sub_dt, dt, &mut buffers);
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
    assert_eq!(
        (buffers.pair_builds, buffers.pair_repairs),
        (1, 0),
        "unspent budget should reuse pairs"
    );
    assert!(buffers.pair_budgets[idx].travel > 0.0);
    // Model an impulse spike after the first build. The next admission must
    // see the wall even though it wasn't a candidate in the old list. The
    // only awake body tripped, which is more than a quarter of them, so the
    // list is rebuilt (D-081).
    buffers.proxies[idx].velocity = Vec2::new(15000.0, 0.0);
    assert_eq!(collect_tripped(&config, sub_dt, &mut buffers), 1);
    assert_eq!(buffers.tripped, [idx as u32]);
    substep(&config, sub_dt, dt - sub_dt, &mut buffers, false);
    assert_eq!((buffers.pair_builds, buffers.pair_repairs), (2, 0));
    assert!(buffers.contacts.iter().any(|c| c.a as usize == idx));
    assert!(
        buffers.proxies[idx].center.x + 4.0 <= 46.5,
        "budget trip tunneled wall"
    );
    let radius = 15000.0 * (dt - sub_dt) + (PAIR_MARGIN_SLOPS + 2.0) * config.linear_slop;
    assert!(
        (buffers.pair_budgets[idx].radius - radius).abs() < 1e-4,
        "rebuild must use the remaining frame time"
    );
    // Accumulated travel is independently sufficient to trip even after
    // velocity drops to zero (e.g. a solver/sweep clamp).
    buffers.proxies[idx].velocity = Vec2::ZERO;
    buffers.pair_budgets[idx].travel = buffers.pair_budgets[idx].radius;
    collect_tripped(&config, sub_dt, &mut buffers);
    assert_eq!(buffers.tripped, [idx as u32]);
}

#[test]
fn gravity_allowance_is_the_exact_worst_case_of_a_falling_body() {
    // D-081: replay the integrator (velocity first, then position) for a
    // body falling along gravity. Before every substep the budget check adds
    // one substep of travel at the current speed to the travel so far; the
    // allowance must cover each check and be tight at the frame's last one.
    let config = PhysicsConfig {
        gravity: Vec2::new(0.0, 900.0),
        ..PhysicsConfig::default()
    };
    let (speed, dt) = (350.0_f32, 1.0_f32 / 60.0);
    for substeps in 1..=8 {
        let h = dt / substeps as f32;
        let allowance = gravity_allowance(&config, h, dt);
        let (mut velocity, mut travel, mut worst) = (speed, 0.0_f32, 0.0_f32);
        for _ in 0..substeps {
            let needed = travel + velocity * h - speed * dt;
            assert!(needed <= allowance + 1e-4, "{substeps} substeps: {needed}");
            worst = needed;
            velocity += 900.0 * h;
            travel += velocity * h;
        }
        assert!(
            (worst - allowance).abs() < 1e-4,
            "{substeps} substeps: last check needs {worst}, allowance {allowance}"
        );
        // D-075's `g t^2 / 2` differs by `g h^2 (n - 2) / 2`: one `g h^2`
        // short at four substeps, which tripped every falling body.
        let old = 0.5 * 900.0 * dt * dt;
        let shortfall = 900.0 * h * h * (substeps as f32 - 2.0) / 2.0;
        assert!((allowance - old - shortfall).abs() < 1e-4, "{substeps}");
    }
    // A repair late in the frame predicts only what is left.
    let h = dt / 4.0;
    assert_eq!(gravity_allowance(&config, h, h), 0.0);
    assert!((gravity_allowance(&config, h, 2.0 * h) - 2.0 * 900.0 * h * h).abs() < 1e-6);
}

/// `cols` by `rows` dynamic circles of radius 5 on a 9.5 px lattice, in
/// proxy order row by row. Neighbours overlap by 0.5 px, so every contact
/// carries a small impulse while the bodies creep apart at about 2 px/s, far
/// below what a budget needs to trip.
fn spawn_lattice(world: &mut World, cols: usize, rows: usize) {
    for i in 0..cols * rows {
        let e = world.spawn();
        world.insert(
            e,
            Position(Vec2::new((i % cols) as f32, (i / cols) as f32) * 9.5),
        );
        world.insert(e, Velocity(Vec2::ZERO));
        world.insert(e, Collider::circle(5.0));
        world.insert(e, RigidBody::dynamic());
    }
}

/// Gathered buffers with both oracles armed, for driving substeps by hand:
/// the contact set against per-substep pair finding (D-075) and every warm
/// start against the per-substep keyed map (D-076).
fn oracle_buffers(world: &World) -> PhysicsBuffers {
    let mut buffers = PhysicsBuffers::default();
    gather_proxies(world, &mut buffers.proxies);
    buffers.island_parent = (0..buffers.proxies.len() as u32).collect();
    buffers.pairs_invalidated = true;
    buffers.check_pair_contacts = true;
    buffers.reference_impulses = Some(ImpulseMap::default());
    buffers
}

/// The carried impulse of pair `(a, b)`, which must be in the list once and
/// in that orientation.
fn carried_impulse(buffers: &PhysicsBuffers, a: u32, b: u32) -> f32 {
    let mut found = buffers
        .pairs
        .iter()
        .zip(&buffers.pair_impulses)
        .filter(|&(&(x, y), _)| (x, y) == (a, b) || (x, y) == (b, a));
    let (&pair, &impulse) = found.next().expect("pair missing from the list");
    assert_eq!(pair, (a, b), "pair stored in the wrong orientation");
    assert!(found.next().is_none(), "pair ({a}, {b}) listed twice");
    impulse
}

#[test]
fn tripped_bodies_are_repaired_without_a_rebuild() {
    // D-081 on a 5 by 4 lattice (index = row * 5 + column), one frame driven
    // by hand with both oracles checking every substep.
    let mut world = seed_world();
    spawn_lattice(&mut world, 5, 4);
    let config = *world.get_resource::<PhysicsConfig>().unwrap();
    let dt = 1.0 / 60.0;
    let sub_dt = dt / 4.0;
    let mut buffers = oracle_buffers(&world);
    substep(&config, sub_dt, dt, &mut buffers, false);
    assert_eq!((buffers.pair_builds, buffers.pair_repairs), (1, 0));
    let before = buffers.pairs.clone();
    let kept_impulse = carried_impulse(&buffers, 7, 8);
    let left_impulse = carried_impulse(&buffers, 8, 9);
    let above_impulse = carried_impulse(&buffers, 4, 9);
    let member_impulse = carried_impulse(&buffers, 9, 14);
    let below_impulse = carried_impulse(&buffers, 14, 19);
    for impulse in [kept_impulse, left_impulse, above_impulse, member_impulse] {
        assert!(impulse > 0.0, "lattice contacts must carry an impulse");
    }

    // Two neighbours on the right edge speed up and leave. Two of twenty
    // awake bodies tripped, so their pairs are repaired.
    buffers.proxies[9].velocity = Vec2::new(300.0, 0.0);
    buffers.proxies[14].velocity = Vec2::new(300.0, 0.0);
    refresh_pairs(&config, sub_dt, dt - sub_dt, &mut buffers);
    assert_eq!((buffers.pair_builds, buffers.pair_repairs), (1, 1));
    assert_eq!(buffers.tripped, [9, 14]);
    assert_eq!(buffers.repaired, [9, 14]);
    // Pairs without a member keep their order at the front of the list.
    let kept: Vec<_> = before
        .iter()
        .copied()
        .filter(|&(a, b)| ![9, 14].contains(&a) && ![9, 14].contains(&b))
        .collect();
    assert_eq!(buffers.pairs[..kept.len()], kept[..]);
    assert_eq!(carried_impulse(&buffers, 7, 8), kept_impulse);
    // An untripped awake neighbour of lower index: the build's initiator
    // rule would drop these two pairs, since only the members query.
    assert_eq!(carried_impulse(&buffers, 8, 9), left_impulse);
    assert_eq!(carried_impulse(&buffers, 4, 9), above_impulse);
    // A pair between two members is added once, by the lower one.
    assert_eq!(carried_impulse(&buffers, 9, 14), member_impulse);
    assert_eq!(carried_impulse(&buffers, 14, 19), below_impulse);
    let radius = 300.0 * (dt - sub_dt) + (PAIR_MARGIN_SLOPS + 2.0) * config.linear_slop;
    assert!((buffers.pair_budgets[9].radius - radius).abs() < 1e-4);
    assert_eq!(buffers.pair_budgets[9].travel, 0.0);
    substep(&config, sub_dt, dt - sub_dt, &mut buffers, false);
    assert_eq!((buffers.pair_builds, buffers.pair_repairs), (1, 1));

    // A pair between bodies repaired in different substeps: 13 trips now,
    // and finds 14 and 9 through the repair grid.
    buffers.proxies[13].velocity = Vec2::new(300.0, 0.0);
    refresh_pairs(&config, sub_dt, dt - 2.0 * sub_dt, &mut buffers);
    assert_eq!(buffers.tripped, [13]);
    assert_eq!(buffers.repaired, [9, 14, 13]);
    carried_impulse(&buffers, 13, 14);
    carried_impulse(&buffers, 9, 13);
    carried_impulse(&buffers, 12, 13);
    substep(&config, sub_dt, dt - 2.0 * sub_dt, &mut buffers, false);

    // A body repaired twice in one frame stays in the repaired list once.
    buffers.proxies[9].velocity = Vec2::new(900.0, 0.0);
    refresh_pairs(&config, sub_dt, dt - 3.0 * sub_dt, &mut buffers);
    assert_eq!(buffers.tripped, [9]);
    assert_eq!(buffers.repaired, [9, 14, 13]);
    substep(&config, sub_dt, dt - 3.0 * sub_dt, &mut buffers, false);
    assert_eq!((buffers.pair_builds, buffers.pair_repairs), (1, 3));
    assert_eq!(buffers.checked_substeps, 4);
}

#[test]
fn more_than_a_quarter_tripped_rebuilds_instead_of_repairing() {
    let mut world = seed_world();
    spawn_lattice(&mut world, 4, 2);
    let config = *world.get_resource::<PhysicsConfig>().unwrap();
    let dt = 1.0 / 60.0;
    let sub_dt = dt / 4.0;
    let mut buffers = oracle_buffers(&world);
    substep(&config, sub_dt, dt, &mut buffers, false);

    // Two of eight is a quarter: still a repair.
    for index in [3, 7] {
        buffers.proxies[index].velocity = Vec2::new(300.0, 0.0);
    }
    substep(&config, sub_dt, dt - sub_dt, &mut buffers, false);
    assert_eq!((buffers.pair_builds, buffers.pair_repairs), (1, 1));

    // Three of eight is more: the list is rebuilt, and the build forgets
    // which proxies were repaired.
    for index in [0, 3, 4] {
        buffers.proxies[index].velocity = Vec2::new(-900.0, 0.0);
    }
    substep(&config, sub_dt, dt - 2.0 * sub_dt, &mut buffers, false);
    assert_eq!((buffers.pair_builds, buffers.pair_repairs), (2, 1));
    assert!(buffers.repaired.is_empty());
    assert!(buffers.pair_flags.iter().all(|&flags| flags == PAIR_AWAKE));
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
    build_pairs(&config, 1.0 / 240.0, 1.0, &mut buffers);
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

fn arrival_counts(world: &World) -> (usize, usize) {
    let buffers = world
        .get_resource::<PhysicsBuffers>()
        .expect("physics buffers");
    (buffers.arrival.listed, buffers.arrival.clamps)
}

#[test]
fn settled_stack_never_enters_the_arrival_pass() {
    // Warm starts and gravity cancel in a resting pile, so no body's velocity
    // moves by the arrival threshold and the pass lists nobody (D-092).
    let mut world = seed_world();
    {
        let config = world.get_resource_mut::<PhysicsConfig>().unwrap();
        config.gravity = Vec2::new(0.0, 900.0);
        config.sleep_threshold = 0.0;
    }
    let floor = world.spawn();
    world.insert(floor, Position(Vec2::new(0.0, 500.0)));
    world.insert(floor, Collider::aabb(Vec2::new(400.0, 20.0)));
    world.insert(floor, RigidBody::r#static());
    for i in 0..5 {
        let body = world.spawn();
        world.insert(body, Position(Vec2::new(0.0, 464.0 - 32.0 * i as f32)));
        world.insert(body, Velocity(Vec2::ZERO));
        world.insert(body, Collider::aabb(Vec2::splat(16.0)));
        world.insert(body, RigidBody::dynamic());
    }

    for _ in 0..240 {
        physics_step(&mut world);
    }
    let settled = arrival_counts(&world);
    for _ in 0..120 {
        physics_step(&mut world);
    }
    assert_eq!(arrival_counts(&world), settled);
}

#[test]
fn pushed_body_arrives_at_a_static_wall_and_stops_its_pusher() {
    // A heavy pusher drives a light body at a thin static wall. The wall
    // contact is solved before or after the push depending on spawn order;
    // either way the body ends against the wall and the pusher behind it.
    const RADIUS: f32 = 8.0;
    const WALL_FACE: f32 = 798.0;
    for wall_first in [false, true] {
        let mut world = seed_world();
        let spawn_wall = |world: &mut World| {
            let wall = world.spawn();
            world.insert(wall, Position(Vec2::new(WALL_FACE + 2.0, 0.0)));
            world.insert(wall, Collider::aabb(Vec2::new(2.0, 400.0)));
            world.insert(wall, RigidBody::r#static());
        };
        let spawn_ball = |world: &mut World, x: f32, speed: f32, mass: f32| {
            let ball = world.spawn();
            world.insert(ball, Position(Vec2::new(x, 0.0)));
            world.insert(ball, Velocity(Vec2::new(speed, 0.0)));
            world.insert(ball, Collider::circle(RADIUS));
            world.insert(ball, RigidBody::dynamic().with_mass(mass));
            ball
        };
        if wall_first {
            spawn_wall(&mut world);
        }
        let pushed = spawn_ball(&mut world, WALL_FACE - RADIUS - 3.0, 0.0, 1.0);
        if !wall_first {
            spawn_wall(&mut world);
        }
        let pusher = spawn_ball(&mut world, WALL_FACE - RADIUS - 3.0 - 46.0, 1_920.0, 100.0);

        for _ in 0..60 {
            physics_step(&mut world);
        }

        let pushed_x = world.get::<Position>(pushed).unwrap().0.x;
        let pusher_x = world.get::<Position>(pusher).unwrap().0.x;
        assert!(
            pushed_x + RADIUS <= WALL_FACE + 1.0,
            "wall_first={wall_first}: pushed body sits {} px into the wall",
            pushed_x + RADIUS - WALL_FACE
        );
        assert!(
            pusher_x + 2.0 * RADIUS <= pushed_x + 1.0,
            "wall_first={wall_first}: pusher at {pusher_x} overlaps the pushed body at {pushed_x}"
        );
        let (listed, clamps) = arrival_counts(&world);
        assert!(listed > 0 && clamps > 0, "the arrival pass did not run");
    }
}

/// Two bodies in free fall with nothing in reach, one per integration path:
/// a collider body (substeps) and a collider-less one (`integrate_loose_bodies`).
fn spawn_free_fallers(world: &mut World, x: f32) -> [Entity; 2] {
    let ball = world.spawn();
    world.insert(ball, Position(Vec2::new(x, 0.0)));
    world.insert(ball, Velocity(Vec2::new(60.0, 0.0)));
    world.insert(ball, Collider::circle(7.5));
    world.insert(ball, RigidBody::dynamic());
    let loose = world.spawn();
    world.insert(loose, Position(Vec2::new(x + 200.0, 0.0)));
    world.insert(loose, Velocity(Vec2::new(60.0, 0.0)));
    world.insert(loose, RigidBody::dynamic());
    [ball, loose]
}

fn body_bits(world: &World, bodies: &[Entity]) -> Vec<u32> {
    bodies
        .iter()
        .flat_map(|&body| {
            let position = world.get::<Position>(body).unwrap().0;
            let velocity = world.get::<Velocity>(body).unwrap().0;
            [position.x, position.y, velocity.x, velocity.y].map(f32::to_bits)
        })
        .collect()
}

/// Step-bound scene (D-094): a five-ball column on a static floor under
/// gravity, still awake when the measured steps begin, with the two free
/// fallers beside it. Ten steps at 1/60 s, then 20 of `dt`; returns every
/// body's position and velocity bits.
fn step_bound_scene(max_step_dt: f32, dt: f32) -> Vec<u32> {
    const RADIUS: f32 = 7.5;
    let mut world = seed_world();
    {
        let config = world.get_resource_mut::<PhysicsConfig>().unwrap();
        config.gravity = Vec2::new(0.0, 900.0);
        config.max_step_dt = max_step_dt;
    }
    let floor = world.spawn();
    world.insert(floor, Position(Vec2::new(0.0, 520.0)));
    world.insert(floor, Collider::aabb(Vec2::new(100.0, 20.0)));
    world.insert(floor, RigidBody::r#static());
    let mut bodies: Vec<Entity> = (0..5)
        .map(|i| {
            let body = world.spawn();
            let y = 500.0 - RADIUS - 2.0 * RADIUS * i as f32;
            world.insert(body, Position(Vec2::new(0.0, y)));
            world.insert(body, Velocity(Vec2::ZERO));
            world.insert(body, Collider::circle(RADIUS));
            world.insert(body, RigidBody::dynamic());
            body
        })
        .collect();
    bodies.extend(spawn_free_fallers(&mut world, 300.0));

    for _ in 0..10 {
        physics_step(&mut world);
    }
    world.get_resource_mut::<DeltaTime>().unwrap().dt = dt;
    for _ in 0..20 {
        physics_step(&mut world);
    }
    body_bits(&world, &bodies)
}

#[test]
fn slow_frame_advances_only_the_step_bound() {
    // D-094: handed the 0.1 s dt cap of D-088, each call advances
    // `max_step_dt` and drops the rest, on both integration paths.
    const STEPS: usize = 3;
    let bound = PhysicsConfig::default().max_step_dt;
    let run = |dt: f32| {
        let mut world = seed_world();
        world.get_resource_mut::<PhysicsConfig>().unwrap().gravity = Vec2::new(0.0, 900.0);
        let fallers = spawn_free_fallers(&mut world, 0.0);
        world.get_resource_mut::<DeltaTime>().unwrap().dt = dt;
        for _ in 0..STEPS {
            physics_step(&mut world);
        }
        let state = fallers.map(|body| {
            (
                world.get::<Position>(body).unwrap().0,
                world.get::<Velocity>(body).unwrap().0,
            )
        });
        (state, body_bits(&world, &fallers))
    };

    let (slow, slow_bits) = run(0.1);
    let (_, bound_bits) = run(bound);
    assert_eq!(slow_bits, bound_bits, "a 0.1 s dt is not a {bound} s step");

    let elapsed = STEPS as f32 * bound;
    for (start_x, (position, velocity)) in [0.0, 200.0].into_iter().zip(slow) {
        assert!(
            (position.x - start_x - 60.0 * elapsed).abs() < 1.0e-3,
            "advanced {} px in x, not {elapsed} s of 60 px/s",
            position.x - start_x
        );
        assert!(
            (velocity.y - 900.0 * elapsed).abs() < 1.0e-2,
            "fell to {} px/s, not {elapsed} s of gravity",
            velocity.y
        );
    }
}

#[test]
fn step_bound_off_takes_the_whole_dt_as_before() {
    // `max_step_dt <= 0` is unbounded. The hash (FNV-1a over the scene's
    // position and velocity bits) is from the tree before the bound existed.
    const BEFORE_D094: u64 = 0x070b_6b82_ad91_b1fb;
    let hash = |bits: &[u32]| {
        bits.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, &bits| {
            (hash ^ u64::from(bits)).wrapping_mul(0x0000_0100_0000_01b3)
        })
    };
    for off in [0.0, -1.0] {
        let unbounded = hash(&step_bound_scene(off, 0.1));
        assert_eq!(
            unbounded, BEFORE_D094,
            "max_step_dt = {off}: {unbounded:#018x}"
        );
    }
    let bounded = hash(&step_bound_scene(PhysicsConfig::default().max_step_dt, 0.1));
    assert_ne!(
        bounded, BEFORE_D094,
        "the default bound left a 0.1 s step alone"
    );
}

#[test]
fn step_bound_is_invisible_at_a_sixtieth() {
    // The pinned 1/60 s step of benchmarks, smoke runs and the pixel test
    // (D-088) must not see the bound: default and off agree bit for bit.
    let bound = PhysicsConfig::default().max_step_dt;
    assert_eq!(
        step_bound_scene(bound, 1.0 / 60.0),
        step_bound_scene(0.0, 1.0 / 60.0)
    );
}
